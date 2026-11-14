use agora_wire::local_handshake::ServerIncarnationV1;
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::FileTypeExt;

/// Maximum entropy reads used to obtain a valid incarnation.
///
/// The bound prevents a faulty or adversarial source from creating an
/// unbounded startup loop. An all-zero value and, for recovery, the immediately
/// prior incarnation are rejected.
pub const INCARNATION_GENERATION_ATTEMPTS_V1: usize = 8;

/// Injectable entropy boundary for server-incarnation generation.
pub trait ServerIncarnationEntropyV1 {
    fn fill(&mut self, output: &mut [u8; 16]) -> io::Result<()>;
}

/// Unix operating-system entropy read from the character device
/// `/dev/urandom`.
///
/// Opening and reading are explicit effects. The source grants no authority,
/// persistence, uniqueness proof, or restart policy.
pub struct OsServerIncarnationEntropyV1 {
    source: File,
}

impl OsServerIncarnationEntropyV1 {
    pub fn open() -> Result<Self, IncarnationGenerationErrorV1> {
        let source = File::open("/dev/urandom")
            .map_err(|error| IncarnationGenerationErrorV1::Entropy(error.kind()))?;
        let metadata = source
            .metadata()
            .map_err(|error| IncarnationGenerationErrorV1::Entropy(error.kind()))?;
        if !metadata.file_type().is_char_device() {
            return Err(IncarnationGenerationErrorV1::InvalidSource);
        }
        Ok(Self { source })
    }
}

impl std::fmt::Debug for OsServerIncarnationEntropyV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OsServerIncarnationEntropyV1")
    }
}

impl ServerIncarnationEntropyV1 for OsServerIncarnationEntropyV1 {
    fn fill(&mut self, output: &mut [u8; 16]) -> io::Result<()> {
        self.source.read_exact(output)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncarnationGenerationErrorV1 {
    Entropy(io::ErrorKind),
    InvalidSource,
    Exhausted,
}

/// Generates one nonzero server incarnation from an injected entropy source.
pub fn generate_server_incarnation_v1(
    entropy: &mut impl ServerIncarnationEntropyV1,
) -> Result<ServerIncarnationV1, IncarnationGenerationErrorV1> {
    generate_excluding(entropy, None)
}

/// Generates one nonzero server incarnation distinct from the supplied prior
/// lifetime.
pub fn generate_distinct_server_incarnation_v1(
    entropy: &mut impl ServerIncarnationEntropyV1,
    previous: ServerIncarnationV1,
) -> Result<ServerIncarnationV1, IncarnationGenerationErrorV1> {
    generate_excluding(entropy, Some(previous))
}

fn generate_excluding(
    entropy: &mut impl ServerIncarnationEntropyV1,
    excluded: Option<ServerIncarnationV1>,
) -> Result<ServerIncarnationV1, IncarnationGenerationErrorV1> {
    for _ in 0..INCARNATION_GENERATION_ATTEMPTS_V1 {
        let mut bytes = [0_u8; 16];
        entropy
            .fill(&mut bytes)
            .map_err(|error| IncarnationGenerationErrorV1::Entropy(error.kind()))?;
        let candidate = ServerIncarnationV1::new(bytes);
        if !candidate.is_zero() && excluded != Some(candidate) {
            return Ok(candidate);
        }
    }
    Err(IncarnationGenerationErrorV1::Exhausted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct SequenceEntropy {
        values: VecDeque<io::Result<[u8; 16]>>,
        calls: usize,
    }

    impl SequenceEntropy {
        fn new(values: impl IntoIterator<Item = io::Result<[u8; 16]>>) -> Self {
            Self {
                values: values.into_iter().collect(),
                calls: 0,
            }
        }
    }

    impl ServerIncarnationEntropyV1 for SequenceEntropy {
        fn fill(&mut self, output: &mut [u8; 16]) -> io::Result<()> {
            self.calls += 1;
            let value = self.values.pop_front().expect("bounded test entropy");
            output.copy_from_slice(&value?);
            Ok(())
        }
    }

    #[test]
    fn zero_is_retried_before_a_nonzero_value_is_returned() {
        let expected = ServerIncarnationV1::new([0x5a; 16]);
        let mut entropy = SequenceEntropy::new([Ok([0; 16]), Ok(expected.as_bytes())]);

        assert_eq!(generate_server_incarnation_v1(&mut entropy), Ok(expected));
        assert_eq!(entropy.calls, 2);
    }

    #[test]
    fn recovery_generation_retries_the_immediately_prior_value() {
        let previous = ServerIncarnationV1::new([0x5a; 16]);
        let next = ServerIncarnationV1::new([0xa5; 16]);
        let mut entropy = SequenceEntropy::new([Ok(previous.as_bytes()), Ok(next.as_bytes())]);

        assert_eq!(
            generate_distinct_server_incarnation_v1(&mut entropy, previous),
            Ok(next)
        );
        assert_eq!(entropy.calls, 2);
    }

    #[test]
    fn invalid_values_exhaust_at_the_fixed_bound() {
        let mut entropy =
            SequenceEntropy::new((0..INCARNATION_GENERATION_ATTEMPTS_V1).map(|_| Ok([0; 16])));

        assert_eq!(
            generate_server_incarnation_v1(&mut entropy),
            Err(IncarnationGenerationErrorV1::Exhausted)
        );
        assert_eq!(entropy.calls, INCARNATION_GENERATION_ATTEMPTS_V1);
    }

    #[test]
    fn entropy_failure_is_erased_to_its_content_free_kind() {
        let mut entropy = SequenceEntropy::new([Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "sensitive source detail",
        ))]);

        let error = generate_server_incarnation_v1(&mut entropy).unwrap_err();
        assert_eq!(
            error,
            IncarnationGenerationErrorV1::Entropy(io::ErrorKind::PermissionDenied)
        );
        assert!(!format!("{error:?}").contains("sensitive"));
    }

    #[test]
    fn qualified_os_source_yields_a_nonzero_value() {
        let mut entropy = OsServerIncarnationEntropyV1::open().unwrap();
        let incarnation = generate_server_incarnation_v1(&mut entropy).unwrap();

        assert!(!incarnation.is_zero());
        assert_eq!(format!("{entropy:?}"), "OsServerIncarnationEntropyV1");
    }
}
