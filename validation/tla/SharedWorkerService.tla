------------------------- MODULE SharedWorkerService -------------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
 * One renderer worker serves every output of a DRM device group. Until      *
 * this module, each physical head owned its own worker thread, its own EGL  *
 * display, and its own copy of every imported renderer image; coalescing    *
 * them onto one thread is what this models.                                 *
 *                                                                           *
 * A per-head worker could answer one question by position: the next result  *
 * on the channel was the answer to the only request outstanding, and a      *
 * request id that did not match was a fault. Sharing the thread destroys    *
 * that. Two outputs now have work in the same queue, so a result must say   *
 * whose it is, a render for one output must not consume the other's slot    *
 * or lease, and neither output may be passed over indefinitely because the  *
 * other keeps producing.                                                    *
 *                                                                           *
 * What this module does NOT model, deliberately: slot incarnations and      *
 * stale releases, which VisualRetirementSlots already owns and which do not *
 * change here; and any timing property, because the harness holds that no   *
 * model expresses a deadline. Skew is therefore a count of how many times   *
 * one output may be served ahead of another with work waiting -- structure, *
 * not milliseconds.                                                         *
 *************************************************************************)

CONSTANTS Outputs, MaxGenerations, NoOutput, NoGeneration

ASSUME
    /\ Outputs # {}
    /\ NoOutput \notin Outputs
    /\ MaxGenerations \in Nat
    /\ NoGeneration = 0

(***************************************************************************
 * pending      : each output's one latest-wins request cell, a generation   *
 *                or NoGeneration. A newer scene overwrites an unserviced    *
 *                one rather than queueing behind it.                        *
 * queue        : the shared command channel, in service order.              *
 * rendering    : which output the worker is currently rendering for.        *
 * renderingGen : the generation it is rendering.                            *
 * reply        : each output's own bounded reply cell. Routing means a      *
 *                result lands here and nowhere else.                        *
 * inFlight     : the exporter-side view -- an output with a request out.    *
 * served       : how many renders each output has completed.                *
 * passedOver   : how many times another output has been served while this   *
 *                one already had a submitted request waiting. Comparing     *
 *                raw service counts instead was the first thing TLC broke   *
 *                here: an output with nothing to draw legitimately falls    *
 *                arbitrarily behind, and calling that skew makes the        *
 *                invariant fire on a healthy idle head.                     *
 * misrouted    : set TRUE if a result ever reaches the wrong output.        *
 * budget       : bounds the environment so liveness is a question about     *
 *                arbitration rather than about outrunning a producer.       *
 *************************************************************************)
VARIABLES pending, queue, rendering, renderingGen, reply, inFlight, served,
          passedOver, misrouted, budget

vars == <<pending, queue, rendering, renderingGen, reply, inFlight, served,
          passedOver, misrouted, budget>>

Generations == 1..MaxGenerations

Init ==
    /\ pending = [o \in Outputs |-> NoGeneration]
    /\ queue = <<>>
    /\ rendering = NoOutput
    /\ renderingGen = NoGeneration
    /\ reply = [o \in Outputs |-> NoGeneration]
    /\ inFlight = [o \in Outputs |-> FALSE]
    /\ served = [o \in Outputs |-> 0]
    /\ passedOver = [o \in Outputs |-> 0]
    /\ misrouted = FALSE
    /\ budget = MaxGenerations

(***************************************************************************
 * Environment. Composition offers a new generation for one output. It is    *
 * unfair and budgeted: nothing obliges a client to draw.                    *
 *                                                                           *
 * Composing while that output already has a render in flight is ordinary,   *
 * not excluded: the pending cell is filled without consulting the worker,   *
 * which is the whole reason supersession exists. An earlier draft forbade   *
 * it, and the effect was to make the submission gate below unreachable --   *
 * its negative control could not fail, because the environment was already  *
 * doing the gate's job.                                                     *
 *************************************************************************)
Compose(o, g) ==
    /\ budget > 0
    /\ pending[o] = NoGeneration
    /\ pending' = [pending EXCEPT ![o] = g]
    /\ budget' = budget - 1
    /\ UNCHANGED <<queue, rendering, renderingGen, reply, inFlight, served,
         passedOver, misrouted>>

(***************************************************************************
 * Latest-wins supersession. A second generation for an output whose earlier *
 * one has not been submitted replaces it in place. This is the property the *
 * row calls "one latest pending request per output": the cell never grows,  *
 * and one output's backlog can never become a queue that starves another.   *
 *************************************************************************)
Supersede(o, g) ==
    /\ budget > 0
    /\ pending[o] # NoGeneration
    /\ pending[o] # g
    /\ pending' = [pending EXCEPT ![o] = g]
    /\ budget' = budget - 1
    /\ UNCHANGED <<queue, rendering, renderingGen, reply, inFlight, served,
         passedOver, misrouted>>

(***************************************************************************
 * Submission. The exporter hands its pending generation to the shared       *
 * queue. The per-output gate is what keeps one output from occupying the    *
 * worker twice; it is the shared-thread analogue of the single in_flight    *
 * cell each per-head worker used to hold.                                   *
 *************************************************************************)
Submit(o) ==
    /\ pending[o] # NoGeneration
    /\ ~inFlight[o]
    /\ reply[o] = NoGeneration
    /\ queue' = Append(queue, <<o, pending[o]>>)
    /\ inFlight' = [inFlight EXCEPT ![o] = TRUE]
    /\ pending' = [pending EXCEPT ![o] = NoGeneration]
    /\ UNCHANGED <<rendering, renderingGen, reply, served, passedOver,
         misrouted, budget>>

QueueEntriesFor(o) ==
    Cardinality({i \in DOMAIN queue : queue[i][1] = o})

(***************************************************************************
 * The worker takes the head of the queue. One render at a time on one       *
 * thread: that is the whole point of coalescing, and it is why service      *
 * order rather than parallelism is what has to be fair.                     *
 *                                                                           *
 * Taking the head is also the entire skew argument. Every output waiting at  *
 * this moment has been passed over once more; the one now served is square.  *
 *************************************************************************)
BeginRender ==
    /\ rendering = NoOutput
    /\ queue # <<>>
    /\ rendering' = Head(queue)[1]
    /\ renderingGen' = Head(queue)[2]
    /\ queue' = Tail(queue)
    /\ passedOver' = [o \in Outputs |->
           IF o = Head(queue)[1] THEN 0
           ELSE IF QueueEntriesFor(o) > 0 THEN passedOver[o] + 1
           ELSE passedOver[o]]
    /\ UNCHANGED <<pending, reply, inFlight, served, misrouted, budget>>

(***************************************************************************
 * The render completes and the result is routed to the output that asked.   *
 * Delivering into reply[rendering] is the model of the per-output reply      *
 * channel; a worker that instead answered whoever polled first would set     *
 * misrouted, which is the fault this module exists to forbid.                *
 *************************************************************************)
RouteTarget == rendering

FinishRender ==
    /\ rendering # NoOutput
    /\ reply' = [reply EXCEPT ![RouteTarget] = renderingGen]
    /\ served' = [served EXCEPT ![rendering] = served[rendering] + 1]
    /\ misrouted' = (misrouted \/ (RouteTarget # rendering) \/ ~inFlight[rendering])
    /\ rendering' = NoOutput
    /\ renderingGen' = NoGeneration
    /\ UNCHANGED <<pending, queue, inFlight, passedOver, budget>>

(***************************************************************************
 * The exporter collects its own reply, freeing it to submit again.          *
 *************************************************************************)
Collect(o) ==
    /\ reply[o] # NoGeneration
    /\ reply' = [reply EXCEPT ![o] = NoGeneration]
    /\ inFlight' = [inFlight EXCEPT ![o] = FALSE]
    /\ UNCHANGED <<pending, queue, rendering, renderingGen, served, passedOver,
         misrouted, budget>>

ServiceProgress ==
    \/ \E o \in Outputs : Submit(o)
    \/ BeginRender
    \/ FinishRender
    \/ \E o \in Outputs : Collect(o)

Next ==
    \/ \E o \in Outputs, g \in Generations : Compose(o, g)
    \/ \E o \in Outputs, g \in Generations : Supersede(o, g)
    \/ ServiceProgress

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(ServiceProgress)

TypeOK ==
    /\ pending \in [Outputs -> Generations \cup {NoGeneration}]
    /\ rendering \in Outputs \cup {NoOutput}
    /\ renderingGen \in Generations \cup {NoGeneration}
    /\ reply \in [Outputs -> Generations \cup {NoGeneration}]
    /\ inFlight \in [Outputs -> BOOLEAN]
    /\ passedOver \in [Outputs -> 0..Cardinality(Outputs)]
    /\ misrouted \in BOOLEAN
    /\ budget \in 0..MaxGenerations

(***************************************************************************
 * A result belongs to the output that requested it. On a per-head worker    *
 * this was true by construction; on a shared one it is a claim that has to  *
 * be checked, and it is the reason the reply cells are per output at all.   *
 *************************************************************************)
ResponsesRouteToTheirOutput == ~misrouted

(***************************************************************************
 * An output occupies the shared worker at most once. Two entries for one    *
 * output would mean a second render against slots the first still holds.    *
 *************************************************************************)
OneInFlightPerOutput ==
    \A o \in Outputs :
        /\ QueueEntriesFor(o) + (IF rendering = o THEN 1 ELSE 0)
             + (IF reply[o] # NoGeneration THEN 1 ELSE 0) <= 1
        /\ (QueueEntriesFor(o) = 1 \/ rendering = o \/ reply[o] # NoGeneration)
             => inFlight[o]

(***************************************************************************
 * The pending cell holds one generation, never a queue. Latest-wins is what *
 * bounds a slow output's memory and keeps its backlog off the shared thread.*
 *************************************************************************)
PendingIsLatestPerOutput ==
    \A o \in Outputs : pending[o] \in Generations \cup {NoGeneration}

(***************************************************************************
 * Bounded inter-output service skew, stated against an output that actually *
 * has work waiting. With one request outstanding per output and FIFO        *
 * service, at most one sibling can be taken ahead of a waiting request       *
 * before it is served, so no output is ever passed over more times than it   *
 * has siblings. A worker that chose its next render by scanning outputs in   *
 * a fixed order instead of taking the queue in order would let a busy first  *
 * output push this past the bound and starve the second.                     *
 *                                                                           *
 * The first version of this compared served counts directly and TLC refuted  *
 * it in fifteen steps, correctly: an output nobody is drawing to falls as    *
 * far behind as the run is long, and that is not skew.                       *
 *************************************************************************)
ServiceSkewBounded ==
    \A o \in Outputs : passedOver[o] < Cardinality(Outputs)

(***************************************************************************
 * Liveness under service fairness only. Work offered to an output is        *
 * eventually rendered for that output; nothing about when.                  *
 *************************************************************************)
EveryOutputIsServed ==
    \A o \in Outputs : (pending[o] # NoGeneration) ~> (reply[o] # NoGeneration)

=============================================================================
