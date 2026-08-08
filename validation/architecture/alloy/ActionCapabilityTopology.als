module ActionCapabilityTopology

abstract sig Role {}
one sig PolicyIssuerRole, BrokerIssuerRole, SessionIssuerRole,
        WmRecipientRole, ShellRecipientRole extends Role {}

abstract sig ActionClass {}
one sig IndicatorPolicyAction, ToplevelMetadataAction, SessionAction
  extends ActionClass {}

sig Epoch {}
sig Slot {}
sig Generation {}
sig ActivationId {}

sig Authority {
  role: one Role
}

sig ActionCapability {
  issuer: one Authority,
  issuerEpoch: one Epoch,
  issuerRevocationEpoch: one Epoch,
  recipient: one Authority,
  recipientEpoch: one Epoch,
  action: one ActionClass,
  targetSlot: lone Slot,
  targetGeneration: lone Generation
}

sig Invocation {
  actor: one Authority,
  actorEpoch: one Epoch,
  capability: one ActionCapability,
  action: one ActionClass,
  targetSlot: lone Slot,
  targetGeneration: lone Generation,
  activation: one ActivationId
}

one sig State {
  live: set Authority,
  currentEpoch: Authority -> one Epoch,
  revocationEpoch: Authority -> one Epoch,
  valid: set ActionCapability,
  revoked: set ActionCapability,
  expired: set ActionCapability,
  accepted: set Invocation
}

pred ActionRequiresTarget[action: ActionClass] {
  action in IndicatorPolicyAction + ToplevelMetadataAction
}

pred IssuerMayIssue[issuer: Authority, action: ActionClass] {
  (action = IndicatorPolicyAction implies issuer.role = PolicyIssuerRole)
  (action = ToplevelMetadataAction implies issuer.role = BrokerIssuerRole)
  (action = SessionAction implies issuer.role = SessionIssuerRole)
}

pred RecipientMayInvoke[recipient: Authority, action: ActionClass] {
  (action in IndicatorPolicyAction + ToplevelMetadataAction implies
    recipient.role = ShellRecipientRole)
  (action = SessionAction implies
    recipient.role in WmRecipientRole + ShellRecipientRole)
}

pred CapabilityWellFormed[cap: ActionCapability] {
  IssuerMayIssue[cap.issuer, cap.action]
  RecipientMayInvoke[cap.recipient, cap.action]
  (one cap.targetSlot) iff (one cap.targetGeneration)
  ActionRequiresTarget[cap.action] implies one cap.targetSlot
}

pred InvocationMatchesCapability[i: Invocation] {
  let cap = i.capability |
    i.actor = cap.recipient and
    i.actorEpoch = cap.recipientEpoch and
    i.action = cap.action and
    i.targetSlot = cap.targetSlot and
    i.targetGeneration = cap.targetGeneration
}

pred CapabilityIsCurrent[cap: ActionCapability] {
  cap in State.valid
  cap not in State.revoked + State.expired
  cap.issuer + cap.recipient in State.live
  State.currentEpoch[cap.issuer] = cap.issuerEpoch
  State.currentEpoch[cap.recipient] = cap.recipientEpoch
  State.revocationEpoch[cap.issuer] = cap.issuerRevocationEpoch
}

pred SecureAcceptance {
  all cap: ActionCapability | cap in State.valid implies CapabilityWellFormed[cap]
  all i: State.accepted |
    InvocationMatchesCapability[i] and CapabilityIsCurrent[i.capability]
  all disj first, second: State.accepted |
    first.actor != second.actor or
    first.actorEpoch != second.actorEpoch or
    first.activation != second.activation
}

assert AcceptedActionsMatchIssuerFamily {
  SecureAcceptance implies
    all i: State.accepted |
      IssuerMayIssue[i.capability.issuer, i.action]
}

assert AcceptedRecipientsMatchActionFamily {
  SecureAcceptance implies
    all i: State.accepted |
      RecipientMayInvoke[i.actor, i.action]
}

assert StaleRevokedOrExpiredCapabilitiesAreRejected {
  SecureAcceptance implies
    no i: State.accepted |
      i.capability in State.revoked + State.expired or
      i.capability.issuer + i.capability.recipient not in State.live or
      State.currentEpoch[i.capability.issuer] != i.capability.issuerEpoch or
      State.currentEpoch[i.capability.recipient] != i.capability.recipientEpoch or
      State.revocationEpoch[i.capability.issuer] !=
        i.capability.issuerRevocationEpoch
}

assert AcceptedTargetsMatchCapability {
  SecureAcceptance implies
    all i: State.accepted |
      i.targetSlot = i.capability.targetSlot and
      i.targetGeneration = i.capability.targetGeneration
}

assert AcceptedActivationIdentitiesDoNotReplay {
  SecureAcceptance implies
    no disj first, second: State.accepted |
      first.actor = second.actor and
      first.actorEpoch = second.actorEpoch and
      first.activation = second.activation
}

pred ValidScopedAction {
  SecureAcceptance
  some State.accepted
}

pred CrossIssuerTypeConfusionAttack {
  some i: State.accepted |
    i.action = SessionAction and
    i.capability.issuer.role = BrokerIssuerRole
}

pred WrongRecipientRoleAttack {
  some i: State.accepted |
    i.action = ToplevelMetadataAction and
    i.actor.role = WmRecipientRole
}

pred StaleCapabilityAttack {
  some i: State.accepted |
    State.currentEpoch[i.capability.recipient] != i.capability.recipientEpoch
}

pred RevokedCapabilityAttack {
  some i: State.accepted | i.capability in State.revoked
}

pred TargetGenerationSubstitutionAttack {
  some i: State.accepted |
    i.targetSlot = i.capability.targetSlot and
    i.targetGeneration != i.capability.targetGeneration
}

pred RecipientSubstitutionAttack {
  some i: State.accepted | i.actor != i.capability.recipient
}

pred ActivationReplayAttack {
  some disj first, second: State.accepted |
    first.actor = second.actor and
    first.actorEpoch = second.actorEpoch and
    first.activation = second.activation
}

check AcceptedActionsMatchIssuerFamily for 7
check AcceptedRecipientsMatchActionFamily for 7
check StaleRevokedOrExpiredCapabilitiesAreRejected for 7
check AcceptedTargetsMatchCapability for 7
check AcceptedActivationIdentitiesDoNotReplay for 7
run ValidScopedAction for 7
run CrossIssuerTypeConfusionAttack for 7
run WrongRecipientRoleAttack for 7
run StaleCapabilityAttack for 7
run RevokedCapabilityAttack for 7
run TargetGenerationSubstitutionAttack for 7
run RecipientSubstitutionAttack for 7
run ActivationReplayAttack for 7
