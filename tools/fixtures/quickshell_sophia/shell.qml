// The panel the Quickshell smoke runs, kept minimal on purpose.
//
// A shell is a different class of X client from the terminals and GL demos the
// matrix proves: it is a dock rather than a normal window, it reserves work
// area with a strut, and it presents to a long-lived surface. Those are the
// three things this file exists to exercise, so it does nothing else -- a
// widget that failed for its own reasons would make the probe say less, not
// more.
import Quickshell
import QtQuick

ShellRoot {
  PanelWindow {
    anchors {
      top: true
      left: true
      right: true
    }
    implicitHeight: 32
    color: "#1e1e2e"

    SystemClock {
      id: clock
      precision: SystemClock.Seconds
    }

    Text {
      anchors.centerIn: parent
      color: "#cdd6f4"
      font.pixelSize: 16
      // Moving text, so a frame that never changes is visible as one.
      text: Qt.formatDateTime(clock.date, "HH:mm:ss")
    }
  }
}
