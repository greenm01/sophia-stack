---------------------- MODULE PageFlipCompletionPump ----------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************
 * Focused model of the production KMS completion boundary.                *
 *                                                                         *
 * Rust correspondence:                                                   *
 * - production_session/native_scanout.rs::pump_native_completions reads   *
 *   each card and routes one bounded ledger slot per physical head;       *
 * - production_visual_runtime/service.rs::service_native pumps, retires,  *
 *   and only then evaluates the hard-stall watchdog;                      *
 * - drm/native_scanout/prepare.rs captures OUT_FENCE_PTR with the affine   *
 *   scanout owner; and                                                    *
 * - production_session/native_scanout.rs quarantines page-flip events     *
 *   after a head selects authoritative out-fence completion.              *
 *                                                                         *
 * Deliberately absent: DRM handles, rendering, timestamps, topology       *
 * preparation, and timeout duration. Expiry is an environment fact.       *
 *************************************************************************)

CONSTANTS Heads, MaxGeneration

ASSUME Heads # {} /\ MaxGeneration \in Nat /\ MaxGeneration >= 1

Modes == {"page_flip", "out_fence"}
Phases == {"idle", "pump", "retire"}
Generations == 0..MaxGeneration

VARIABLES inFlight, lastRetired, retired, retireCount,
          eventReady, ledger, fenceReady, mode, expired, watchdog,
          lateEvents, phase

vars == <<inFlight, lastRetired, retired, retireCount,
          eventReady, ledger, fenceReady, mode, expired, watchdog,
          lateEvents, phase>>

Init ==
    /\ inFlight = [h \in Heads |-> 0]
    /\ lastRetired = [h \in Heads |-> 0]
    /\ retired = {}
    /\ retireCount = [h \in Heads |-> 0]
    /\ eventReady = {}
    /\ ledger = {}
    /\ fenceReady = {}
    /\ mode = [h \in Heads |-> "page_flip"]
    /\ expired = {}
    /\ watchdog = {}
    /\ lateEvents = {}
    /\ phase = "idle"

(***************************************************************************
 * A head owns one outstanding generation. A successor is available only   *
 * after exact retirement advances lastRetired.                             *
 *************************************************************************)
Submit(h) ==
    /\ phase = "idle"
    /\ watchdog = {}
    /\ inFlight[h] = 0
    /\ lastRetired[h] < MaxGeneration
    /\ inFlight' = [inFlight EXCEPT ![h] = lastRetired[h] + 1]
    /\ UNCHANGED <<lastRetired, retired, retireCount, eventReady, ledger,
         fenceReady, mode, expired, watchdog, lateEvents, phase>>

KernelEvent(h) ==
    /\ inFlight[h] # 0
    /\ h \notin watchdog
    /\ h \notin eventReady
    /\ eventReady' = eventReady \cup {h}
    /\ UNCHANGED <<inFlight, lastRetired, retired, retireCount, ledger,
         fenceReady, mode, expired, watchdog, lateEvents, phase>>

FenceSignal(h) ==
    /\ inFlight[h] # 0
    /\ h \notin watchdog
    /\ h \notin fenceReady
    /\ fenceReady' = fenceReady \cup {h}
    /\ UNCHANGED <<inFlight, lastRetired, retired, retireCount, eventReady,
         ledger, mode, expired, watchdog, lateEvents, phase>>

Expire(h) ==
    /\ inFlight[h] # 0
    /\ h \notin expired
    /\ expired' = expired \cup {h}
    /\ UNCHANGED <<inFlight, lastRetired, retired, retireCount, eventReady,
         ledger, fenceReady, mode, watchdog, lateEvents, phase>>

BeginService ==
    /\ phase = "idle"
    /\ phase' = "pump"
    /\ UNCHANGED <<inFlight, lastRetired, retired, retireCount, eventReady,
         ledger, fenceReady, mode, expired, watchdog, lateEvents>>

(***************************************************************************
 * One card pump collects all ready head events before any retirement or    *
 * watchdog action. In out-fence mode an event is late evidence: count it   *
 * and do not put it in a retirement ledger.                                *
 *************************************************************************)
PumpCard ==
    /\ \/ phase = "pump"
       \/ phase = "retire" /\ eventReady # {}
    /\ ledger' = ledger \cup {h \in eventReady : mode[h] = "page_flip"}
    /\ lateEvents' = lateEvents \cup
         {h \in eventReady : mode[h] = "out_fence"}
    /\ eventReady' = {}
    /\ phase' = "retire"
    /\ UNCHANGED <<inFlight, lastRetired, retired, retireCount,
         fenceReady, mode, expired, watchdog>>

(***************************************************************************
 * A page-flip ledger is preferred. If it is absent and the owned fence is  *
 * ready, the fence retires this exact generation and becomes authoritative *
 * for successors. Both actions are head-scoped.                            *
 *************************************************************************)
RetirePageFlip(h) ==
    /\ phase = "retire"
    /\ inFlight[h] # 0
    /\ h \in ledger
    /\ <<h, inFlight[h]>> \notin retired
    /\ retired' = retired \cup {<<h, inFlight[h]>>}
    /\ retireCount' = [retireCount EXCEPT ![h] = @ + 1]
    /\ lastRetired' = [lastRetired EXCEPT ![h] = inFlight[h]]
    /\ inFlight' = [inFlight EXCEPT ![h] = 0]
    /\ ledger' = ledger \ {h}
    /\ fenceReady' = fenceReady \ {h}
    /\ expired' = expired \ {h}
    /\ UNCHANGED <<eventReady, mode, watchdog, lateEvents, phase>>

RetireOutFence(h) ==
    /\ phase = "retire"
    /\ inFlight[h] # 0
    /\ h \notin ledger
    /\ h \in fenceReady
    /\ <<h, inFlight[h]>> \notin retired
    /\ retired' = retired \cup {<<h, inFlight[h]>>}
    /\ retireCount' = [retireCount EXCEPT ![h] = @ + 1]
    /\ lastRetired' = [lastRetired EXCEPT ![h] = inFlight[h]]
    /\ inFlight' = [inFlight EXCEPT ![h] = 0]
    /\ fenceReady' = fenceReady \ {h}
    /\ expired' = expired \ {h}
    /\ mode' = [mode EXCEPT ![h] = "out_fence"]
    /\ UNCHANGED <<eventReady, ledger, watchdog, lateEvents, phase>>

ReadyToRetire(h) ==
    inFlight[h] # 0 /\ (h \in ledger \/ h \in fenceReady)

(***************************************************************************
 * The watchdog is evaluated only after the card pump and every available   *
 * proof have been retired. It records only expired heads still lacking a   *
 * completion proof.                                                        *
 *************************************************************************)
CompleteService ==
    /\ phase = "retire"
    /\ eventReady = {}
    /\ \A h \in Heads : ~ReadyToRetire(h)
    /\ watchdog' = watchdog \cup
         {h \in Heads : h \in expired /\ inFlight[h] # 0}
    /\ phase' = "idle"
    /\ UNCHANGED <<inFlight, lastRetired, retired, retireCount, eventReady,
         ledger, fenceReady, mode, expired, lateEvents>>

Quiesce ==
    /\ phase = "idle"
    /\ UNCHANGED vars

Next ==
    \/ \E h \in Heads : Submit(h)
    \/ \E h \in Heads : KernelEvent(h)
    \/ \E h \in Heads : FenceSignal(h)
    \/ \E h \in Heads : Expire(h)
    \/ BeginService
    \/ PumpCard
    \/ \E h \in Heads : RetirePageFlip(h)
    \/ \E h \in Heads : RetireOutFence(h)
    \/ CompleteService
    \/ Quiesce

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ inFlight \in [Heads -> Generations]
    /\ lastRetired \in [Heads -> Generations]
    /\ retired \subseteq Heads \X (1..MaxGeneration)
    /\ retireCount \in [Heads -> Generations]
    /\ eventReady \subseteq Heads
    /\ ledger \subseteq Heads
    /\ fenceReady \subseteq Heads
    /\ mode \in [Heads -> Modes]
    /\ expired \subseteq Heads
    /\ watchdog \subseteq Heads
    /\ lateEvents \subseteq Heads
    /\ phase \in Phases

OneSubmissionPerHead ==
    \A h \in Heads : inFlight[h] = 0 \/ inFlight[h] = lastRetired[h] + 1

ExactRetirement ==
    \A h \in Heads :
        /\ retireCount[h] = Cardinality({pair \in retired : pair[1] = h})
        /\ lastRetired[h] = retireCount[h]

RetiredIdentityIsHeadScoped ==
    \A pair \in retired : pair[2] <= lastRetired[pair[1]]

NoFalseWatchdog ==
    \A h \in watchdog :
        /\ h \in expired
        /\ inFlight[h] # 0
        /\ h \notin eventReady
        /\ h \notin ledger
        /\ h \notin fenceReady

LateEventsCannotRetireSuccessors ==
    \A h \in Heads : mode[h] = "out_fence" => h \notin ledger

=============================================================================
