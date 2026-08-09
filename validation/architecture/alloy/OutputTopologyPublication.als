module OutputTopologyPublication

sig Epoch {}
sig RawOutput {}

sig OutputIdentity {
  raw: one RawOutput,
  generation: one Int
} {
  generation > 0
}

sig Topology {
  epoch: one Epoch,
  outputs: set OutputIdentity
}

abstract sig Consumer {}
one sig Scanout, PointerBounds, RandR, PolicySnapshot extends Consumer {}

one sig State {
  previous: one Topology,
  current: one Topology,
  published: Consumer -> one Topology,
  policyCommitted: one Topology,
  presented: one Topology,
  inputEnabled: one Int
} {
  inputEnabled in 0 + 1
}

pred UniqueIdentityGenerations {
  all disj first, second: OutputIdentity |
    first.raw = second.raw implies first.generation != second.generation
}

pred CompleteTopology[topology: Topology] {
  some topology.outputs
  all disj first, second: topology.outputs | first.raw != second.raw
}

pred SecurePublication {
  State.previous.epoch != State.current.epoch
  CompleteTopology[State.previous]
  CompleteTopology[State.current]
  UniqueIdentityGenerations
  all consumer: Consumer | State.published[consumer] = State.current
  State.inputEnabled = 1 implies
    (State.policyCommitted = State.current and State.presented = State.current)
}

assert AllConsumersShareCompleteEpoch {
  SecurePublication implies
    all disj first, second: Consumer |
      State.published[first].epoch = State.published[second].epoch and
      State.published[first].outputs = State.published[second].outputs
}

assert InputUsesPresentedCurrentTopology {
  SecurePublication implies
    State.inputEnabled = 1 implies
      (State.policyCommitted = State.current and State.presented = State.current)
}

assert OutputIdentityGenerationIsUnique {
  SecurePublication implies
    all disj first, second: OutputIdentity |
      first.raw = second.raw implies first.generation != second.generation
}

assert PublishedTopologyIsNonempty {
  SecurePublication implies
    all consumer: Consumer | some State.published[consumer].outputs
}

pred ValidPublication {
  SecurePublication
  State.inputEnabled = 1
  State.previous.outputs != State.current.outputs
}

pred MixedConsumerEpochAttack {
  State.previous.epoch != State.current.epoch
  CompleteTopology[State.previous]
  CompleteTopology[State.current]
  State.published[Scanout] = State.current
  State.published[PointerBounds] = State.current
  State.published[PolicySnapshot] = State.current
  State.published[RandR] = State.previous
}

pred PartialOutputPublicationAttack {
  State.previous.epoch != State.current.epoch
  CompleteTopology[State.current]
  State.published[Scanout] = State.current
  some partial: Topology |
    partial.epoch = State.current.epoch and
    partial.outputs in State.current.outputs and
    partial.outputs != State.current.outputs and
    State.published[PolicySnapshot] = partial
}

pred StalePresentedInputAttack {
  State.previous.epoch != State.current.epoch
  CompleteTopology[State.previous]
  CompleteTopology[State.current]
  all consumer: Consumer | State.published[consumer] = State.current
  State.presented = State.previous
  State.inputEnabled = 1
}

pred UncommittedPolicyInputAttack {
  State.previous.epoch != State.current.epoch
  CompleteTopology[State.previous]
  CompleteTopology[State.current]
  all consumer: Consumer | State.published[consumer] = State.current
  State.policyCommitted = State.previous
  State.presented = State.current
  State.inputEnabled = 1
}

pred ReusedOutputGenerationAttack {
  some disj first, second: OutputIdentity |
    first.raw = second.raw and first.generation = second.generation
}

pred EmptyPublishedTopologyAttack {
  no State.current.outputs
  all consumer: Consumer | State.published[consumer] = State.current
}

check AllConsumersShareCompleteEpoch for 7 but 4 Int
check InputUsesPresentedCurrentTopology for 7 but 4 Int
check OutputIdentityGenerationIsUnique for 7 but 4 Int
check PublishedTopologyIsNonempty for 7 but 4 Int
run ValidPublication for 7 but 4 Int
run MixedConsumerEpochAttack for 7 but 4 Int
run PartialOutputPublicationAttack for 7 but 4 Int
run StalePresentedInputAttack for 7 but 4 Int
run UncommittedPolicyInputAttack for 7 but 4 Int
run ReusedOutputGenerationAttack for 7 but 4 Int
run EmptyPublishedTopologyAttack for 7 but 4 Int
