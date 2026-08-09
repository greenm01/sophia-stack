module PolicyOperationBinding

sig Action {}
sig Slot {}
sig Token {}

sig Binding {
  action: one Action,
  operationSlot: lone Slot
}

sig SessionOperation {
  slot: one Slot,
  token: one Token,
  permitsTarget: one Int
} {
  permitsTarget in 0 + 1
}

sig Invocation {
  action: one Action,
  binding: one Binding,
  operation: one SessionOperation,
  token: one Token,
  carriesTarget: one Int
} {
  carriesTarget in 0 + 1
}

one sig State {
  bindings: set Binding,
  operations: set SessionOperation,
  accepted: set Invocation
}

pred SecureOperationAdmission {
  all disj first, second: State.bindings | first.action != second.action
  all disj first, second: State.operations | first.slot != second.slot
  all invocation: State.accepted |
    invocation.binding in State.bindings and
    invocation.operation in State.operations and
    invocation.action = invocation.binding.action and
    invocation.binding.operationSlot = invocation.operation.slot and
    invocation.token = invocation.operation.token and
    (invocation.carriesTarget = 1 implies
      invocation.operation.permitsTarget = 1)
}

assert AcceptedOperationUsesCommittedBindingSlot {
  SecureOperationAdmission implies
    all invocation: State.accepted |
      invocation.binding.operationSlot = invocation.operation.slot
}

assert AcceptedOperationUsesAdvertisedOpaqueToken {
  SecureOperationAdmission implies
    all invocation: State.accepted |
      invocation.token = invocation.operation.token
}

assert TargetDeliveryRequiresPermission {
  SecureOperationAdmission implies
    no invocation: State.accepted |
      invocation.carriesTarget = 1 and invocation.operation.permitsTarget = 0
}

pred ValidOperation {
  SecureOperationAdmission
  some State.accepted
}

pred NumericActionInferenceAttack {
  all disj first, second: State.bindings | first.action != second.action
  all disj first, second: State.operations | first.slot != second.slot
  all invocation: State.accepted |
    invocation.binding in State.bindings and
    invocation.operation in State.operations and
    invocation.action = invocation.binding.action and
    invocation.token = invocation.operation.token and
    (invocation.carriesTarget = 1 implies
      invocation.operation.permitsTarget = 1)
  some invocation: State.accepted |
    no invocation.binding.operationSlot or
    invocation.binding.operationSlot != invocation.operation.slot
}

pred OpaqueTokenSubstitutionAttack {
  all disj first, second: State.bindings | first.action != second.action
  all disj first, second: State.operations | first.slot != second.slot
  all invocation: State.accepted |
    invocation.binding in State.bindings and
    invocation.operation in State.operations and
    invocation.action = invocation.binding.action and
    invocation.binding.operationSlot = invocation.operation.slot and
    (invocation.carriesTarget = 1 implies
      invocation.operation.permitsTarget = 1)
  some invocation: State.accepted |
    invocation.token != invocation.operation.token
}

pred UnauthorizedTargetAttack {
  all disj first, second: State.bindings | first.action != second.action
  all disj first, second: State.operations | first.slot != second.slot
  all invocation: State.accepted |
    invocation.binding in State.bindings and
    invocation.operation in State.operations and
    invocation.action = invocation.binding.action and
    invocation.binding.operationSlot = invocation.operation.slot and
    invocation.token = invocation.operation.token
  some invocation: State.accepted |
    invocation.carriesTarget = 1 and invocation.operation.permitsTarget = 0
}

check AcceptedOperationUsesCommittedBindingSlot for 6 but 2 Int
check AcceptedOperationUsesAdvertisedOpaqueToken for 6 but 2 Int
check TargetDeliveryRequiresPermission for 6 but 2 Int
run ValidOperation for 6 but 2 Int
run NumericActionInferenceAttack for 6 but 2 Int
run OpaqueTokenSubstitutionAttack for 6 but 2 Int
run UnauthorizedTargetAttack for 6 but 2 Int
