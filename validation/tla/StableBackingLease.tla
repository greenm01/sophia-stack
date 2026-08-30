---------------------- MODULE StableBackingLease ----------------------
EXTENDS Integers, FiniteSets

(***************************************************************************
 * Copy-on-write backing for one stable software-rendered X toplevel.       *
 *                                                                          *
 * `VisualDamageHistory` owns what a reused target slot already contains.    *
 * This model is the same shape one level down, and one property further:    *
 * what the renderer's registry copy of a client's backing contains, given    *
 * updates applied while presentations still hold the bytes.                 *
 *                                                                          *
 * That difference is why this is a separate model rather than an extension. *
 * `VisualDamageHistory` states it has no lease incarnation, because a slot's *
 * buffer keeps its content across release and reacquisition and that        *
 * persistence is what makes buffer age worth anything. A client raster      *
 * under copy-on-write does not have that property: a live lease is exactly   *
 * what decides whether a mutation may touch the allocation or must split it. *
 *                                                                          *
 * `VisualRetirementSlots` owns lease token identity and the ABA-shaped      *
 * stale return; this borrows its discipline, not its slot vocabulary.       *
 * `SurfaceContentStream` owns commit ordering.                              *
 *                                                                          *
 * Scenario correspondence and the implementation-only checks live in        *
 * validation/specula/stable-x-backing-lease-modeling-brief.md.              *
 *                                                                          *
 * Content is a generation mark per abstract region, never pixels, and never *
 * rectangles: an update is modelled by the region set it covers, so a       *
 * coalesced batch is a superset cover and a full replacement covers         *
 * everything. The model governs what an update must cover, not how the      *
 * encoder chooses to say it.                                                *
 *************************************************************************)

CONSTANTS Generations, Regions, Leases, Allocations, NoGeneration, NoAllocation

ASSUME /\ Generations # {}
       /\ Generations \subseteq (Nat \ {0})
       /\ Regions # {}
       /\ Leases # {}
       /\ Allocations # {}
       /\ NoGeneration = 0
       /\ NoAllocation \notin Allocations

MaxGeneration == Cardinality(Generations)
GenerationMarks == Generations \cup {NoGeneration}
Contents == [Regions -> GenerationMarks]

VARIABLES
    store,
    storeGeneration,
    damageOf,
    allocContent,
    allocLive,
    registryAlloc,
    registryGeneration,
    leaseAlloc,
    leaseContent,
    handleEpoch,
    splitAdmitted

vars == <<
    store,
    storeGeneration,
    damageOf,
    allocContent,
    allocLive,
    registryAlloc,
    registryGeneration,
    leaseAlloc,
    leaseContent,
    handleEpoch,
    splitAdmitted
>>

Blank == [r \in Regions |-> NoGeneration]

(***************************************************************************
 * What the registry owes for a generation: the union of every generation's  *
 * damage after the one its copy was written for. A copy that has never been *
 * written owes every region.                                                *
 *************************************************************************)
AccumulatedDamage ==
    UNION { damageOf[g] : g \in { h \in Generations :
                                    /\ h > registryGeneration
                                    /\ h =< storeGeneration } }

HeldLeases == { l \in Leases : leaseAlloc[l] # NoAllocation }

(***************************************************************************
 * Somebody other than the registry still reads these bytes. This is the     *
 * condition `Arc::strong_count > 1` implements; the counter is not modelled. *
 *************************************************************************)
Shared(a) == \E l \in Leases : leaseAlloc[l] = a

FreeAllocations == { a \in Allocations : ~allocLive[a] }

Init ==
    /\ store = Blank
    /\ storeGeneration = NoGeneration
    /\ damageOf = [g \in Generations |-> {}]
    /\ \E a \in Allocations :
        /\ allocContent = [b \in Allocations |-> Blank]
        /\ allocLive = [b \in Allocations |-> b = a]
        /\ registryAlloc = a
    /\ registryGeneration = NoGeneration
    /\ leaseAlloc = [l \in Leases |-> NoAllocation]
    /\ leaseContent = [l \in Leases |-> Blank]
    /\ handleEpoch = 1
    /\ splitAdmitted = FALSE

(***************************************************************************
 * The client draws. A new generation changes a nonempty set of regions in    *
 * the authority's presentation buffer. Whether the registry ever catches up  *
 * is a separate question, which is what makes an update that is superseded   *
 * different from one that never happened.                                    *
 *************************************************************************)
AdvanceStore ==
    /\ storeGeneration < MaxGeneration
    /\ \E d \in (SUBSET Regions) \ {{}} :
        LET g == storeGeneration + 1 IN
        /\ store' = [r \in Regions |-> IF r \in d THEN g ELSE store[r]]
        /\ damageOf' = [damageOf EXCEPT ![g] = d]
        /\ storeGeneration' = g
    /\ UNCHANGED << allocContent, allocLive, registryAlloc, registryGeneration,
                    leaseAlloc, leaseContent, handleEpoch, splitAdmitted >>

(***************************************************************************
 * An update covering `cover` is applied to the registry's own allocation.    *
 * Admissible only when nothing else holds those bytes: a lease captured what *
 * it captured and may not be edited underneath.                              *
 *************************************************************************)
ApplyInPlace ==
    /\ storeGeneration > registryGeneration
    /\ ~Shared(registryAlloc)
    /\ \E cover \in { c \in SUBSET Regions : AccumulatedDamage \subseteq c } :
        allocContent' = [allocContent EXCEPT ![registryAlloc] =
            [r \in Regions |-> IF r \in cover THEN store[r]
                               ELSE allocContent[registryAlloc][r]]]
    /\ registryGeneration' = storeGeneration
    /\ UNCHANGED << store, storeGeneration, damageOf, allocLive, registryAlloc,
                    leaseAlloc, leaseContent, handleEpoch, splitAdmitted >>

(***************************************************************************
 * The same update where a lease does hold the bytes: copy them into a free   *
 * allocation, apply there, and read from the copy. The lease keeps reading   *
 * what it was handed.                                                        *
 *************************************************************************)
ApplyWithSplit ==
    /\ storeGeneration > registryGeneration
    /\ Shared(registryAlloc)
    /\ FreeAllocations # {}
    /\ \E fresh \in FreeAllocations :
        \E cover \in { c \in SUBSET Regions : AccumulatedDamage \subseteq c } :
            /\ allocContent' = [allocContent EXCEPT ![fresh] =
                    [r \in Regions |-> IF r \in cover THEN store[r]
                                       ELSE allocContent[registryAlloc][r]]]
            /\ allocLive' = [allocLive EXCEPT ![fresh] = TRUE]
            /\ registryAlloc' = fresh
    /\ registryGeneration' = storeGeneration
    /\ splitAdmitted' = TRUE
    /\ UNCHANGED << store, storeGeneration, damageOf, leaseAlloc, leaseContent,
                    handleEpoch >>

(***************************************************************************
 * A presentation is planned against the registry's current bytes and holds   *
 * them until its cohort retires. Capturing the content at acquire is what     *
 * lets immutability be checked rather than assumed.                           *
 *************************************************************************)
AcquireLease(l) ==
    /\ leaseAlloc[l] = NoAllocation
    /\ leaseAlloc' = [leaseAlloc EXCEPT ![l] = registryAlloc]
    /\ leaseContent' = [leaseContent EXCEPT ![l] = allocContent[registryAlloc]]
    /\ UNCHANGED << store, storeGeneration, damageOf, allocContent, allocLive,
                    registryAlloc, registryGeneration, handleEpoch,
                    splitAdmitted >>

(***************************************************************************
 * Retirement or rejection. The allocation is reclaimed only when no other     *
 * lease holds it and the registry has moved off it; otherwise it stays live.  *
 *************************************************************************)
RetireLease(l) ==
    /\ leaseAlloc[l] # NoAllocation
    /\ LET a == leaseAlloc[l] IN
        /\ leaseAlloc' = [leaseAlloc EXCEPT ![l] = NoAllocation]
        /\ leaseContent' = [leaseContent EXCEPT ![l] = Blank]
        /\ allocLive' = [allocLive EXCEPT ![a] =
                \/ a = registryAlloc
                \/ \E m \in Leases \ {l} : leaseAlloc[m] = a]
    /\ UNCHANGED << store, storeGeneration, damageOf, allocContent,
                    registryAlloc, registryGeneration, handleEpoch,
                    splitAdmitted >>

(***************************************************************************
 * An update older than the registry already holds. It must change nothing:   *
 * a stale generation cannot overwrite a newer one, which is the refusal the   *
 * registry makes on generation order.                                         *
 *************************************************************************)
DeliverStale ==
    /\ \E g \in Generations : g < registryGeneration
    /\ UNCHANGED vars

(***************************************************************************
 * The toplevel resizes. A new handle epoch starts from a fresh allocation at  *
 * full cover; the old epoch's bytes survive exactly as long as a lease holds   *
 * them, which is the case a plan measured against the old size depends on.     *
 *************************************************************************)
Resize ==
    /\ handleEpoch < 2
    /\ storeGeneration # NoGeneration
    /\ FreeAllocations # {}
    /\ \E fresh \in FreeAllocations :
        /\ allocContent' = [allocContent EXCEPT ![fresh] = store]
        /\ allocLive' = [allocLive EXCEPT ![fresh] = TRUE,
                         ![registryAlloc] = Shared(registryAlloc)]
        /\ registryAlloc' = fresh
    /\ registryGeneration' = storeGeneration
    /\ handleEpoch' = handleEpoch + 1
    /\ UNCHANGED << store, storeGeneration, damageOf, leaseAlloc, leaseContent,
                    splitAdmitted >>

Next ==
    \/ AdvanceStore
    \/ ApplyInPlace
    \/ ApplyWithSplit
    \/ \E l \in Leases : AcquireLease(l)
    \/ \E l \in Leases : RetireLease(l)
    \/ DeliverStale
    \/ Resize

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ store \in Contents
    /\ storeGeneration \in 0..MaxGeneration
    /\ damageOf \in [Generations -> SUBSET Regions]
    /\ allocContent \in [Allocations -> Contents]
    /\ allocLive \in [Allocations -> BOOLEAN]
    /\ registryAlloc \in Allocations
    /\ registryGeneration \in 0..MaxGeneration
    /\ leaseAlloc \in [Leases -> Allocations \cup {NoAllocation}]
    /\ leaseContent \in [Leases -> Contents]
    /\ handleEpoch \in 1..2
    /\ splitAdmitted \in BOOLEAN

(***************************************************************************
 * Historical handles stay immutable until their last presentation lease      *
 * retires. A lease reads what it was handed, for as long as it holds it.      *
 * An in-place write under a live lease violates this and nothing else.        *
 *************************************************************************)
LeasedContentStable ==
    \A l \in Leases :
        leaseAlloc[l] # NoAllocation =>
            allocContent[leaseAlloc[l]] = leaseContent[l]

(***************************************************************************
 * A registry brought up to the current generation holds what a full           *
 * replacement would have produced. An update whose cover missed a region       *
 * violates this: the copy is stale in that region while remaining              *
 * self-consistent, presentable, and correctly generation-ordered.              *
 *************************************************************************)
RegistryMatchesStore ==
    registryGeneration = storeGeneration => allocContent[registryAlloc] = store

(***************************************************************************
 * Copy-on-write may split, but only for a holder. One allocation for the      *
 * registry plus at most one per held lease is the whole budget; a split that   *
 * is never reclaimed appears here rather than as growth on a physical run.     *
 *************************************************************************)
AllocationsBounded ==
    Cardinality({ a \in Allocations : allocLive[a] })
        =< 1 + Cardinality(HeldLeases)

(***************************************************************************
 * A lease holds a live allocation. Reclaiming one still being read is the      *
 * same defect as editing it, and shows up here first.                          *
 *************************************************************************)
LeasedAllocationsLive ==
    \A l \in Leases : leaseAlloc[l] # NoAllocation => allocLive[leaseAlloc[l]]

RegistryAllocationLive == allocLive[registryAlloc]

(***************************************************************************
 * The registry never goes backwards, which is the stale-update refusal        *
 * stated as a property rather than as an action.                               *
 *************************************************************************)
RegistryNeverRegresses == registryGeneration =< storeGeneration

(***************************************************************************
 * The optimization has to be reachable, not merely safe. Asserting this        *
 * invariant must produce a counterexample; a model in which no split can ever   *
 * be admitted satisfies every safety property above and describes a system      *
 * that shares nothing and copies always.                                        *
 *************************************************************************)
SplitIsReachable == ~splitAdmitted

=============================================================================
