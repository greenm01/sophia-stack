---------------- MODULE RetainedCompositionAdmission ----------------
EXTENDS Naturals
CONSTANT SuppressedMeansUnavailable
VARIABLES gpuVisible, orderChanged, retainedResult, phase, route
vars == <<gpuVisible, orderChanged, retainedResult, phase, route>>
Init == /\ gpuVisible \in BOOLEAN /\ orderChanged \in BOOLEAN
        /\ retainedResult \in {"queued", "already_owned", "none"}
        /\ (gpuVisible /\ orderChanged => retainedResult # "none")
        /\ phase = "ready" /\ route = "undecided"
Decide == /\ phase = "ready"
          /\ phase' = "decided"
          /\ route' = IF retainedResult = "queued" \/
                         (gpuVisible /\ (~SuppressedMeansUnavailable \/ ~orderChanged))
                       THEN "preserve_native" ELSE "cpu"
          /\ UNCHANGED <<gpuVisible, orderChanged, retainedResult>>
Next == Decide
Spec == Init /\ [][Next]_vars
CpuHasOnlyCpuSources == phase = "decided" /\ route = "cpu" => ~gpuVisible
====================================================================
