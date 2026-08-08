module PresentedTargetTopology

open util/ordering[Target] as TargetOrder
open util/ordering[Trust] as TrustOrder

abstract sig Trust {}
one sig ApplicationTrust, ShellTrust, SecurityTrust extends Trust {}

sig Authority {}
sig Session {}
sig Slot {}
sig Generation {}
sig ModalScope {}
sig Output {}
sig Pixel {
  output: one Output
}
sig Allocation {
  owner: one Authority,
  pixels: some Pixel
}
sig Target {
  owner: one Authority,
  session: one Session,
  slot: one Slot,
  generation: one Generation,
  trust: one Trust,
  modal: one ModalScope,
  pixels: some Pixel
}
sig CoordinateGrant {
  issuer: one Authority,
  recipient: one Authority,
  target: one Target,
  region: some Pixel
}
one sig PresentedSnapshot {
  live: set Authority,
  allocation: set Allocation,
  topOwner: Pixel -> lone Authority,
  activeModal: one ModalScope,
  targets: set Target,
  grants: set CoordinateGrant
}
sig Hit {
  point: one Pixel,
  chosen: one Target
}
one sig Resolution {
  delivered: set Hit
}

fact TrustOrderIsExplicit {
  TrustOrder/first = ApplicationTrust
  TrustOrder/next[ApplicationTrust] = ShellTrust
  TrustOrder/last = SecurityTrust
}

pred OwnedAndVisible[t: Target] {
  t in PresentedSnapshot.targets
  t.owner in PresentedSnapshot.live
  some a: PresentedSnapshot.allocation |
    a.owner = t.owner and t.pixels in a.pixels
  all p: t.pixels | PresentedSnapshot.topOwner[p] = t.owner
}

pred Eligible[t: Target, p: Pixel] {
  OwnedAndVisible[t]
  p in t.pixels
  t.modal = PresentedSnapshot.activeModal
}

pred BetterCandidate[better, current: Target] {
  TrustOrder/lt[current.trust, better.trust] or
  (better.trust = current.trust and TargetOrder/lt[better, current])
}

pred IsWinner[t: Target, p: Pixel] {
  Eligible[t, p]
  no other: Target - t |
    Eligible[other, p] and BetterCandidate[other, t]
}

pred SecurePresentedTopology {
  all disj left, right: PresentedSnapshot.targets |
    left.owner != right.owner or
    left.session != right.session or
    left.slot != right.slot or
    left.generation != right.generation
  all g: PresentedSnapshot.grants {
    g.target in PresentedSnapshot.targets
    g.recipient = g.target.owner
    g.issuer != g.recipient
    g.region in g.target.pixels
  }
  all h: Resolution.delivered | IsWinner[h.chosen, h.point]
}

assert DeliveredTargetsAreOwnedVisibleAndModal {
  SecurePresentedTopology implies
    all h: Resolution.delivered | Eligible[h.chosen, h.point]
}

assert HigherTrustAndTieBreakCannotBeIntercepted {
  SecurePresentedTopology implies
    all h: Resolution.delivered | IsWinner[h.chosen, h.point]
}

assert PresentedTargetIdentitiesAreUnique {
  SecurePresentedTopology implies
    no disj left, right: PresentedSnapshot.targets |
      left.owner = right.owner and
      left.session = right.session and
      left.slot = right.slot and
      left.generation = right.generation
}

assert CoordinateGrantsAreIndependentAndLocal {
  SecurePresentedTopology implies
    all g: PresentedSnapshot.grants |
      g.issuer != g.recipient and g.region in g.target.pixels
}

pred TargetOutsideAllocationAttack {
  some t: PresentedSnapshot.targets |
    no a: PresentedSnapshot.allocation |
      a.owner = t.owner and t.pixels in a.pixels
}

pred OccludedTargetAttack {
  some h: Resolution.delivered |
    h.point in h.chosen.pixels and
    PresentedSnapshot.topOwner[h.point] != h.chosen.owner
}

pred LowerTrustInterceptionAttack {
  some h: Resolution.delivered, higher: Target |
    Eligible[h.chosen, h.point] and
    Eligible[higher, h.point] and
    TrustOrder/lt[h.chosen.trust, higher.trust]
}

pred AmbiguousWithoutTieBreakAttack {
  some p: Pixel, disj left, right: Target |
    Eligible[left, p] and Eligible[right, p] and left.trust = right.trust
}

pred ReusedTargetIdentityAttack {
  some disj left, right: PresentedSnapshot.targets |
    left.owner = right.owner and
    left.session = right.session and
    left.slot = right.slot and
    left.generation = right.generation
}

pred SelfIssuedGrantAttack {
  some g: PresentedSnapshot.grants |
    g.issuer = g.recipient and g.recipient = g.target.owner
}

check DeliveredTargetsAreOwnedVisibleAndModal for 7
check HigherTrustAndTieBreakCannotBeIntercepted for 7
check PresentedTargetIdentitiesAreUnique for 7
check CoordinateGrantsAreIndependentAndLocal for 7
run TargetOutsideAllocationAttack for 7
run OccludedTargetAttack for 7
run LowerTrustInterceptionAttack for 7
run AmbiguousWithoutTieBreakAttack for 7
run ReusedTargetIdentityAttack for 7
run SelfIssuedGrantAttack for 7
