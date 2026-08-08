------------------------ MODULE LegacyWmProjection ------------------------
EXTENDS FiniteSets, Naturals

(***************************************************************************
 * A legacy WM may retain a synthetic X11 window after Sophia changes the  *
 * active workspace. Delayed configure and focus requests remain private   *
 * WM state; translation may expose only requests for the current complete *
 * workspace projection.                                                    *
 *************************************************************************)

CONSTANTS SurfaceOne, SurfaceTwo, WorkspaceOne, WorkspaceTwo, NoSurface

ASSUME /\ SurfaceOne # SurfaceTwo
       /\ WorkspaceOne # WorkspaceTwo
       /\ NoSurface \notin {SurfaceOne, SurfaceTwo}

Surfaces == {SurfaceOne, SurfaceTwo}
Workspaces == {WorkspaceOne, WorkspaceTwo}

VARIABLES activeWorkspace, workspaceMembers, mapped,
          pendingConfigure, pendingFocus,
          configureIssued, focusIssued, lastConfigured, lastFocused

vars == <<activeWorkspace, workspaceMembers, mapped,
          pendingConfigure, pendingFocus,
          configureIssued, focusIssued, lastConfigured, lastFocused>>

Init ==
    /\ activeWorkspace = WorkspaceOne
    /\ workspaceMembers =
        [workspace \in Workspaces |->
            IF workspace = WorkspaceOne THEN {SurfaceOne} ELSE {SurfaceTwo}]
    /\ mapped = {SurfaceOne}
    /\ pendingConfigure = {}
    /\ pendingFocus = {}
    /\ configureIssued = {}
    /\ focusIssued = {}
    /\ lastConfigured = NoSurface
    /\ lastFocused = NoSurface

SwitchWorkspace(workspace) ==
    /\ workspace \in Workspaces
    /\ workspace # activeWorkspace
    /\ activeWorkspace' = workspace
    /\ mapped' = workspaceMembers[workspace]
    /\ lastConfigured' = NoSurface
    /\ lastFocused' = NoSurface
    /\ UNCHANGED <<workspaceMembers, pendingConfigure, pendingFocus,
                    configureIssued, focusIssued>>

ReplaceProjection(workspace, members) ==
    /\ workspace \in Workspaces
    /\ members \subseteq Surfaces
    /\ workspaceMembers' =
        [candidate \in Workspaces |->
            IF candidate = workspace
                THEN members
                ELSE workspaceMembers[candidate] \ members]
    /\ activeWorkspace' = workspace
    /\ mapped' = members
    /\ lastConfigured' = NoSurface
    /\ lastFocused' = NoSurface
    /\ UNCHANGED <<pendingConfigure, pendingFocus,
                    configureIssued, focusIssued>>

AssignWorkspace(surface, workspace) ==
    /\ surface \in Surfaces
    /\ workspace \in Workspaces
    /\ LET nextMembers ==
              [candidate \in Workspaces |->
                  (workspaceMembers[candidate] \ {surface})
                    \cup IF candidate = workspace THEN {surface} ELSE {}]
       IN /\ workspaceMembers' = nextMembers
          /\ mapped' = nextMembers[activeWorkspace]
    /\ lastConfigured' = NoSurface
    /\ lastFocused' = NoSurface
    /\ UNCHANGED <<activeWorkspace, pendingConfigure, pendingFocus,
                    configureIssued, focusIssued>>

IssueConfigure(surface) ==
    /\ surface \in Surfaces \ configureIssued
    /\ configureIssued' = configureIssued \cup {surface}
    /\ pendingConfigure' = pendingConfigure \cup {surface}
    /\ lastConfigured' = NoSurface
    /\ lastFocused' = NoSurface
    /\ UNCHANGED <<activeWorkspace, workspaceMembers, mapped,
                    pendingFocus, focusIssued>>

IssueFocus(surface) ==
    /\ surface \in Surfaces \ focusIssued
    /\ focusIssued' = focusIssued \cup {surface}
    /\ pendingFocus' = pendingFocus \cup {surface}
    /\ lastConfigured' = NoSurface
    /\ lastFocused' = NoSurface
    /\ UNCHANGED <<activeWorkspace, workspaceMembers, mapped, pendingConfigure,
                    configureIssued>>

TranslateConfigure(surface) ==
    /\ surface \in pendingConfigure
    /\ pendingConfigure' = pendingConfigure \ {surface}
    /\ lastConfigured' = IF surface \in mapped THEN surface ELSE NoSurface
    /\ lastFocused' = NoSurface
    /\ UNCHANGED <<activeWorkspace, workspaceMembers, mapped, pendingFocus,
                    configureIssued, focusIssued>>

TranslateFocus(surface) ==
    /\ surface \in pendingFocus
    /\ pendingFocus' = pendingFocus \ {surface}
    /\ lastConfigured' = NoSurface
    /\ lastFocused' = IF surface \in mapped THEN surface ELSE NoSurface
    /\ UNCHANGED <<activeWorkspace, workspaceMembers, mapped, pendingConfigure,
                    configureIssued, focusIssued>>

Progress ==
    \/ \E surface \in pendingConfigure : TranslateConfigure(surface)
    \/ \E surface \in pendingFocus : TranslateFocus(surface)

Next ==
    \/ \E workspace \in Workspaces : SwitchWorkspace(workspace)
    \/ \E workspace \in Workspaces, members \in SUBSET Surfaces :
           ReplaceProjection(workspace, members)
    \/ \E surface \in Surfaces, workspace \in Workspaces :
           AssignWorkspace(surface, workspace)
    \/ \E surface \in Surfaces : IssueConfigure(surface)
    \/ \E surface \in Surfaces : IssueFocus(surface)
    \/ Progress

Spec == Init /\ [][Next]_vars
FairSpec == Spec /\ WF_vars(Progress)

TypeOK ==
    /\ activeWorkspace \in Workspaces
    /\ workspaceMembers \in [Workspaces -> SUBSET Surfaces]
    /\ mapped \subseteq Surfaces
    /\ pendingConfigure \subseteq Surfaces
    /\ pendingFocus \subseteq Surfaces
    /\ configureIssued \subseteq Surfaces
    /\ focusIssued \subseteq Surfaces
    /\ lastConfigured \in Surfaces \cup {NoSurface}
    /\ lastFocused \in Surfaces \cup {NoSurface}

MappedMatchesActiveWorkspace ==
    mapped = workspaceMembers[activeWorkspace]

WorkspaceMembershipIsUnique ==
    \A surface \in Surfaces :
        Cardinality({workspace \in Workspaces :
                         surface \in workspaceMembers[workspace]}) <= 1

TranslatedConfigureIsMapped ==
    lastConfigured = NoSurface \/ lastConfigured \in mapped

TranslatedFocusIsMapped ==
    lastFocused = NoSurface \/ lastFocused \in mapped

PendingRequestsWereIssued ==
    /\ pendingConfigure \subseteq configureIssued
    /\ pendingFocus \subseteq focusIssued

PendingEventuallySettles ==
    (pendingConfigure # {} \/ pendingFocus # {})
        ~> (pendingConfigure = {} /\ pendingFocus = {})

=============================================================================
