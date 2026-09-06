// X11 compatibility fixture, not an implementation of sophia_shell_v1.
import Quickshell
import QtQuick

ShellRoot {
  Variants {
    model: Quickshell.screens
    PanelWindow {
      id: panel
      required property var modelData
      screen: modelData
      anchors { top: true; left: true; right: true }
      implicitHeight: 32
      exclusiveZone: 32
      focusable: false
      color: "#1e1e2e"
      property int count: 0

      SystemClock { id: clock; precision: SystemClock.Seconds }
      Text {
        anchors.centerIn: parent
        color: "#cdd6f4"
        font.pixelSize: 16
        text: Qt.formatDateTime(clock.date, "HH:mm:ss")
      }
      Rectangle {
        width: 130; height: 28
        anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
        color: toggle.containsMouse ? "#585b70" : "#313244"
        Text { anchors.centerIn: parent; color: "#cdd6f4"; text: "Panel test: " + panel.count }
        MouseArea { id: toggle; anchors.fill: parent; hoverEnabled: true; onClicked: popout.visible = !popout.visible }
      }
      PopupWindow {
        id: popout
        anchor.window: panel
        anchor.rect.x: panel.width - width - 8
        anchor.rect.y: panel.height
        implicitWidth: 240
        implicitHeight: 112
        color: "#313244"
        visible: false
        Column {
          anchors.centerIn: parent
          spacing: 8
          Text { color: "#cdd6f4"; text: "Local counter: " + panel.count }
          Rectangle {
            width: 208; height: 28; color: "#45475a"
            Text { anchors.centerIn: parent; color: "#cdd6f4"; text: "Increment" }
            MouseArea { anchors.fill: parent; onClicked: panel.count++ }
          }
          Rectangle {
            width: 208; height: 28; color: "#45475a"
            Text { anchors.centerIn: parent; color: "#cdd6f4"; text: "Close" }
            MouseArea { anchors.fill: parent; onClicked: popout.visible = false }
          }
        }
      }
      // Exercise content and lifecycle without pretending to prove pointer input.
      Timer {
        property int step: 0
        interval: 1000
        repeat: true
        running: Quickshell.env("SOPHIA_PANEL_EXERCISE") === "1"
        onTriggered: {
          step++;
          if (step === 2) popout.visible = true;
          if (step === 4) panel.count++;
          if (step === 6) popout.visible = false;
          if (step === 7) panel.visible = false;
          if (step === 9) panel.visible = true;
          if (step === 11) Qt.quit();
        }
      }
    }
  }
}
