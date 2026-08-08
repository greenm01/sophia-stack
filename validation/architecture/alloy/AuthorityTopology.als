module AuthorityTopology

abstract sig Role {}
one sig EngineRole, WmRole, ShellRole, PortalRole, AppFrontendRole extends Role {}

abstract sig Capability {}
one sig Observe, Mutate, LocalCoordinates extends Capability {}

abstract sig ResourceClass {}
one sig ApplicationMetadata, NamespaceLocal, TargetRegion extends ResourceClass {}

sig Namespace {}
sig ProtectionDomain {}
sig Principal {
  domain: one ProtectionDomain,
  admitted: set Endpoint
}
sig Endpoint {
  role: one Role,
  namespace: lone Namespace
}
sig Resource {
  owner: one Namespace,
  class: one ResourceClass
}
sig PortalGrant {
  issuer: one Endpoint,
  recipient: one Principal,
  resource: one Resource,
  capabilities: some Capability
}
sig Access {
  actor: one Principal,
  endpoint: one Endpoint,
  resource: one Resource,
  capability: one Capability,
  grant: lone PortalGrant
}
one sig Topology {
  delivered: set Access
}

pred DirectNamespaceAccess[a: Access] {
  a.endpoint.namespace = a.resource.owner
  a.capability != LocalCoordinates
}

pred AuthorizedPortalAccess[a: Access] {
  one a.grant
  a.grant.issuer.role = PortalRole
  a.grant.recipient = a.actor
  a.grant.resource = a.resource
  a.capability in a.grant.capabilities
}

pred SecureTopology {
  all a: Topology.delivered {
    a.endpoint in a.actor.admitted
    DirectNamespaceAccess[a] or AuthorizedPortalAccess[a]
    a.capability = LocalCoordinates implies AuthorizedPortalAccess[a]
    a.endpoint.role = WmRole implies a.resource.class != ApplicationMetadata
  }
  no d: ProtectionDomain |
    DomainAdmitsRole[d, WmRole] and
    DomainAdmitsRole[d, ShellRole + PortalRole + AppFrontendRole]
  no d: ProtectionDomain |
    DomainAdmitsRole[d, WmRole] and DomainObserves[d, ApplicationMetadata]
}

pred DomainAdmitsRole[d: ProtectionDomain, roles: set Role] {
  some p: Principal, e: p.admitted |
    p.domain = d and e.role in roles
}

pred DomainObserves[d: ProtectionDomain, resourceClass: ResourceClass] {
  some a: Topology.delivered |
    a.actor.domain = d and a.resource.class = resourceClass
}

assert NoAmbientOrInferredRoleAuthority {
  SecureTopology implies
    all a: Topology.delivered | a.endpoint in a.actor.admitted
}

assert CrossNamespaceAccessRequiresPortalGrant {
  SecureTopology implies
    all a: Topology.delivered |
      a.endpoint.namespace != a.resource.owner implies AuthorizedPortalAccess[a]
}

assert CoordinateAuthorityIsIndependentlyIssued {
  SecureTopology implies
    all a: Topology.delivered |
      a.capability = LocalCoordinates implies a.grant.issuer.role = PortalRole
}

assert WmCannotObserveApplicationMetadata {
  SecureTopology implies
    no a: Topology.delivered |
      a.endpoint.role = WmRole and a.resource.class = ApplicationMetadata
}

assert WmProtectionDomainCannotComposeMetadataRoles {
  SecureTopology implies
    no d: ProtectionDomain |
      DomainAdmitsRole[d, WmRole] and
      DomainAdmitsRole[d, ShellRole + PortalRole + AppFrontendRole]
}

assert WmProtectionDomainCannotObserveApplicationMetadata {
  SecureTopology implies
    no d: ProtectionDomain |
      DomainAdmitsRole[d, WmRole] and DomainObserves[d, ApplicationMetadata]
}

pred AmbientRoleAttack {
  some a: Topology.delivered |
    a.endpoint not in a.actor.admitted and no a.grant
}

pred CrossNamespaceWithoutPortalAttack {
  some a: Topology.delivered |
    a.endpoint.namespace != a.resource.owner and no a.grant
}

pred SelfIssuedCoordinateAttack {
  some a: Topology.delivered |
    a.capability = LocalCoordinates and
    one a.grant and
    a.grant.issuer = a.endpoint and
    a.grant.issuer.role = ShellRole
}

pred WmMetadataAttack {
  some a: Topology.delivered |
    a.endpoint.role = WmRole and a.resource.class = ApplicationMetadata
}

pred CombinedWmShellDomainAttack {
  some d: ProtectionDomain |
    DomainAdmitsRole[d, WmRole] and DomainAdmitsRole[d, ShellRole]
}

pred WmDomainMetadataCollusionAttack {
  some d: ProtectionDomain |
    DomainAdmitsRole[d, WmRole] and DomainObserves[d, ApplicationMetadata]
}

check NoAmbientOrInferredRoleAuthority for 6
check CrossNamespaceAccessRequiresPortalGrant for 6
check CoordinateAuthorityIsIndependentlyIssued for 6
check WmCannotObserveApplicationMetadata for 6
check WmProtectionDomainCannotComposeMetadataRoles for 6
check WmProtectionDomainCannotObserveApplicationMetadata for 6
run AmbientRoleAttack for 6
run CrossNamespaceWithoutPortalAttack for 6
run SelfIssuedCoordinateAttack for 6
run WmMetadataAttack for 6
run CombinedWmShellDomainAttack for 6
run WmDomainMetadataCollusionAttack for 6
