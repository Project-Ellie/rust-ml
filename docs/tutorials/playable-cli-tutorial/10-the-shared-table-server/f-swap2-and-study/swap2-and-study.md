# 10f — Participants, colors, and Study Mode

## Intent

Separate who a person is from which color they currently control. Human
tables now support the complete Swap2 ceremony, freestyle owner color
selection, live authorization, and owner-only Study Mode.

This is the domain slice. It adds little infrastructure; it makes the
server understand the social rules around the existing `SessionCore`.

## Participant is not color

A human table has two stable participants:

- `Owner`
- `Guest`

A normal Gomoku position has two colors:

- Black
- White

Do not identify the owner as Black in storage. In freestyle, the owner's
creation choice establishes the mapping immediately. In Swap2, both
participants stay color-neutral while `OpeningState` conducts the
ceremony. The final `OpeningFinished` event gives the actor enough
information to commit the participant-to-color assignment.

The actor's hosted state therefore contains a `ParticipantAssignment`
that is either unassigned or maps both participants to opposite colors.
`SessionCore` continues to reason in colors; the actor translates an
authenticated participant into the color they may control.

## Live authorization

In Live mode:

- the owner acts only for the owner's assigned color;
- the guest acts only for the guest's assigned color;
- the side to move must match that assignment;
- Swap2 ceremony actions are authorized by ceremony participant, not by
  eventual color;
- seat-management commands that could turn a human seat into AI are not
  public on a human table.

A move can be legal to `SessionCore` yet forbidden for its sender. Actor
authorization runs before core execution. Likewise,
`cli::OccupantKind` is not network identity: when mapping a move event,
the server uses the authenticated participant and hosted mode. This is
especially important when the owner plays the AI-controlled color while
studying.

## Explicit Study Mode

Study Mode is hosted state around the variation tree, not a third player.
Only the owner can enter it. While studying:

- the owner may issue `Play`, `Back`, `Forward`, and `Var` for either
  side;
- the guest remains connected and receives committed SSE events;
- every guest mutation is rejected with 403;
- live participant assignments are preserved but suspended;
- the owner may create and select branches;
- Help and Quit remain local client actions.

`ResumeLive` succeeds only when the cursor is at a branch tip. The
selected tip becomes the new live continuation point, assignments are
restored, and the side to move determines which participant acts next.
If the cursor has children, the owner must navigate to a tip first.

Study entry, study commands, and resumption are journaled. Recovery must
reconstruct the same selected branch and mode.

## Hosted transition table

| State | Principal | Mutation | Result |
|---|---|---|---|
| Forming | owner | close | Closed |
| Opening | ceremony participant | ceremony action | Opening or Live |
| Live | assigned participant | legal move | Live |
| Live | owner | enter study | Study |
| Study | owner | navigate or play | Study |
| Study | guest | any mutation | 403, no revision change |
| Study at tip | owner | resume live | Live |
| Study away from tip | owner | resume live | 422 |
| any active state | owner | close | Closed |

## TDD checklist

1. freestyle creation maps the owner's selected color and the guest's
   opposite color.
2. a participant cannot move the other participant's color in Live mode.
3. Swap2 starts with no color assignment.
4. ceremony authorization follows participant turns.
5. final choice commits one unambiguous participant assignment.
6. owner can enter Study mode from Live.
7. guest receives Study events but every guest mutation returns 403.
8. owner can navigate and play either color in Study mode.
9. resuming away from a tip returns 422 without mutation.
10. resuming at the selected tip restores Live authorization.
11. wire move events identify owner, guest, or hosted AI correctly even
    when `cli::OccupantKind` alone cannot distinguish them.
12. actor eviction and journal replay preserve mode, branch, and
    assignment.
13. local Help and Quit never appear in a server journal.

## Protocol consequences

The protocol needs hosted mutations in addition to serializable core
commands:

- `EnterStudy`
- `ResumeLive`
- `CloseTable`

It also needs snapshot fields for lifecycle state, study state, and
participant assignment. Do not add Study variants to `cli::Command`:
the local core does not know owner and guest identity.

## Gates and capstone

Run protocol serialization tests, server actor and Redis tests, then the
workspace lint and format gates. The capstone completes a two-human
Swap2 opening, proves each participant's assigned color, enters Study
mode, creates a sibling branch while the guest watches, and resumes live
play from the selected tip.
