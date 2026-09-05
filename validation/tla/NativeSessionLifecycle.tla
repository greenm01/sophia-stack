---------------------- MODULE NativeSessionLifecycle ----------------------
EXTENDS Naturals, TLC
CONSTANTS ForgetCounters, ForgetFailures, RequireResume
VARIABLES seat, phase, owner, epoch, submissions, failures, retained, sticky,
          issued, failed, pending, deadline
vars == <<seat, phase, owner, epoch, submissions, failures, retained, sticky,
          issued, failed, pending, deadline>>
Init == /\ seat = "active" /\ phase = "running" /\ owner = TRUE /\ epoch = 1
        /\ submissions = 0 /\ failures = 0 /\ retained = 0 /\ sticky = 0
        /\ issued = 0 /\ failed = 0 /\ pending = 0 /\ deadline = FALSE
Submit == /\ phase = "running" /\ owner /\ issued < 2
          /\ submissions' = submissions + 1 /\ issued' = issued + 1
          /\ pending' = pending + 1
          /\ UNCHANGED <<seat, phase, owner, epoch, failures, retained, sticky,
                         failed, deadline>>
Fail == /\ phase = "running" /\ owner /\ failed = 0
        /\ failures' = failures + 1 /\ failed' = failed + 1
        /\ UNCHANGED <<seat, phase, owner, epoch, submissions, retained, sticky,
                       issued, pending, deadline>>
Settle == /\ pending > 0 /\ pending' = pending - 1
          /\ UNCHANGED <<seat, phase, owner, epoch, submissions, failures,
                         retained, sticky, issued, failed, deadline>>
Close == /\ owner /\ pending = 0
         /\ owner' = FALSE /\ seat' = "suspended"
         /\ retained' = IF ForgetCounters THEN retained ELSE retained + submissions
         /\ sticky' = IF ForgetFailures THEN sticky ELSE sticky + failures
         /\ submissions' = 0 /\ failures' = 0
         /\ UNCHANGED <<phase, epoch, issued, failed, pending, deadline>>
Resume == /\ ~owner /\ phase = "running" /\ ~deadline /\ epoch < 3
          /\ owner' = TRUE /\ seat' = "active" /\ epoch' = epoch + 1
          /\ UNCHANGED <<phase, submissions, failures, retained, sticky,
                         issued, failed, pending, deadline>>
Expire == /\ ~deadline /\ deadline' = TRUE
          /\ UNCHANGED <<seat, phase, owner, epoch, submissions, failures,
                         retained, sticky, issued, failed, pending>>
Shutdown == /\ deadline /\ phase = "running"
            /\ (~RequireResume \/ seat = "active") /\ phase' = "quiescing"
            /\ UNCHANGED <<seat, owner, epoch, submissions, failures,
                           retained, sticky, issued, failed, pending, deadline>>
Finish == /\ phase = "quiescing" /\ pending = 0 /\ phase' = "done"
          /\ UNCHANGED <<seat, owner, epoch, submissions, failures, retained,
                         sticky, issued, failed, pending, deadline>>
Next == Submit \/ Fail \/ Settle \/ Close \/ Resume \/ Expire \/ Shutdown \/ Finish
Spec == Init /\ [][Next]_vars /\ WF_vars(Shutdown) /\ WF_vars(Settle) /\ WF_vars(Finish)
EvidenceRetained == retained + submissions = issued
FailureRetained == sticky + failures = failed
CompletionSettled == phase = "done" => pending = 0
DeadlineCompletes == deadline ~> (phase = "done")
=============================================================================
