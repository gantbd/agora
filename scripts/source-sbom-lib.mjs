const EXPECTED_LICENSE = 'MIT OR Apache-2.0';
const EXPECTED_COMPONENTS = 20;

function refOf(pkg) {
  return `pkg:cargo/${pkg.name}@${pkg.version}`;
}

export function buildSourceSbom(metadata) {
  if (!metadata || !Array.isArray(metadata.packages) || !metadata.resolve) {
    throw new Error('Cargo metadata is incomplete');
  }
  const packages = [...metadata.packages].sort((left, right) => {
    const leftRef = refOf(left);
    const rightRef = refOf(right);
    return leftRef < rightRef ? -1 : leftRef > rightRef ? 1 : 0;
  });
  if (packages.length !== EXPECTED_COMPONENTS) {
    throw new Error(`expected ${EXPECTED_COMPONENTS} packages, found ${packages.length}`);
  }
  const byId = new Map(packages.map((pkg) => [pkg.id, pkg]));
  const components = packages.map((pkg) => {
    if (pkg.source !== null || pkg.license !== EXPECTED_LICENSE || pkg.publish?.length !== 0) {
      throw new Error(`package is outside the qualified local unpublished license profile: ${pkg.name}`);
    }
    return {
      type: 'library',
      'bom-ref': refOf(pkg),
      name: pkg.name,
      version: pkg.version,
      purl: refOf(pkg),
      licenses: [{ expression: pkg.license }],
      properties: [
        { name: 'agora:source-kind', value: 'local-path' },
        { name: 'agora:publication', value: 'denied' },
      ],
    };
  });
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const dependencies = packages.map((pkg) => {
    const node = nodes.get(pkg.id);
    if (!node) throw new Error(`resolved dependency node is absent: ${pkg.name}`);
    const dependsOn = [...new Set(node.deps.map((dep) => {
      const target = byId.get(dep.pkg);
      if (!target) throw new Error(`dependency escaped the stable graph: ${dep.pkg}`);
      return refOf(target);
    }))].sort();
    return { ref: refOf(pkg), dependsOn };
  });
  return {
    bomFormat: 'CycloneDX',
    specVersion: '1.5',
    version: 1,
    components,
    dependencies,
  };
}

function containsHostPath(value) {
  if (typeof value === 'string') {
    return value.includes('file://') || value.includes('/Users/') || value.includes('\\Users\\');
  }
  if (Array.isArray(value)) return value.some(containsHostPath);
  if (value && typeof value === 'object') return Object.values(value).some(containsHostPath);
  return false;
}

export function validateSourceSbom(bom) {
  if (bom?.bomFormat !== 'CycloneDX' || bom?.specVersion !== '1.5' || bom?.version !== 1) {
    throw new Error('SBOM identity is not the pinned CycloneDX 1.5 profile');
  }
  if (!Array.isArray(bom.components) || bom.components.length !== EXPECTED_COMPONENTS) {
    throw new Error('SBOM component inventory is not closed');
  }
  if (!Array.isArray(bom.dependencies) || bom.dependencies.length !== EXPECTED_COMPONENTS) {
    throw new Error('SBOM dependency inventory is not closed');
  }
  if (containsHostPath(bom)) throw new Error('SBOM contains a host path');
  if (JSON.stringify(Object.keys(bom).sort()) !==
      JSON.stringify(['bomFormat', 'components', 'dependencies', 'specVersion', 'version'])) {
    throw new Error('SBOM top-level vocabulary escaped the pinned profile');
  }

  const refs = new Set();
  for (const component of bom.components) {
    if (JSON.stringify(Object.keys(component).sort()) !==
        JSON.stringify(['bom-ref', 'licenses', 'name', 'properties', 'purl', 'type', 'version'])) {
      throw new Error('SBOM component vocabulary escaped the pinned profile');
    }
    if (component.type !== 'library' || component.purl !== component['bom-ref']) {
      throw new Error('SBOM component identity is malformed');
    }
    if (!/^pkg:cargo\/[A-Za-z0-9_-]+@[0-9]+\.[0-9]+\.[0-9]+$/.test(component['bom-ref'])) {
      throw new Error('SBOM component reference is malformed');
    }
    if (refs.has(component['bom-ref'])) throw new Error('SBOM component reference is duplicated');
    refs.add(component['bom-ref']);
    if (component['bom-ref'] !== `pkg:cargo/${component.name}@${component.version}`) {
      throw new Error('SBOM component fields disagree with its reference');
    }
    if (component.licenses?.length !== 1 ||
        JSON.stringify(Object.keys(component.licenses[0]).sort()) !== JSON.stringify(['expression']) ||
        component.licenses[0].expression !== EXPECTED_LICENSE) {
      throw new Error('SBOM license expression escaped the qualified profile');
    }
    const expectedProperties = [
      { name: 'agora:source-kind', value: 'local-path' },
      { name: 'agora:publication', value: 'denied' },
    ];
    if (JSON.stringify(component.properties) !== JSON.stringify(expectedProperties)) {
      throw new Error('SBOM source/publication properties are incomplete');
    }
  }
  const componentRefs = bom.components.map((component) => component['bom-ref']);
  if (JSON.stringify([...componentRefs].sort()) !== JSON.stringify(componentRefs)) {
    throw new Error('SBOM components are not canonically ordered');
  }
  const dependencyRefs = new Set();
  for (const dependency of bom.dependencies) {
    if (JSON.stringify(Object.keys(dependency).sort()) !== JSON.stringify(['dependsOn', 'ref'])) {
      throw new Error('SBOM dependency vocabulary escaped the pinned profile');
    }
    if (!refs.has(dependency.ref) || dependencyRefs.has(dependency.ref)) {
      throw new Error('SBOM dependency owner is unknown or duplicated');
    }
    dependencyRefs.add(dependency.ref);
    if (!Array.isArray(dependency.dependsOn) || dependency.dependsOn.some((ref) => !refs.has(ref))) {
      throw new Error('SBOM dependency edge escapes the component inventory');
    }
    const sorted = [...new Set(dependency.dependsOn)].sort();
    if (JSON.stringify(sorted) !== JSON.stringify(dependency.dependsOn)) {
      throw new Error('SBOM dependency edges are duplicated or noncanonical');
    }
  }
  const orderedDependencies = bom.dependencies.map((dependency) => dependency.ref);
  if (JSON.stringify([...orderedDependencies].sort()) !== JSON.stringify(orderedDependencies)) {
    throw new Error('SBOM dependencies are not canonically ordered');
  }
}
