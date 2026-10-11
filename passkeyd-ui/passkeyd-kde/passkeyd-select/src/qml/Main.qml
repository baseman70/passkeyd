import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

// qmllint disable import
import passkeyd.ui.select

// A plain Plasma-style dialog: content on top, a separator and the buttons
// below. What is shown, and every decision, comes from the Rust side through
// `selectionState`; each screen is its own file in this folder.
Controls.ApplicationWindow {
    id: root

    readonly property int fixedWidth: Kirigami.Units.gridUnit * 22
    readonly property int margin: Kirigami.Units.largeSpacing
    // The window is as tall as its content needs and grows when the core adds
    // a notice, so there is never empty space and never a squeezed field.
    readonly property int fittedHeight: content.implicitHeight + margin * 2

    // Same order as the children of the StackLayout below.
    readonly property var pages: ["select", "waiting", "form", "fingerprint"]

    visible: true
    title: selectionState.title

    width: fixedWidth
    height: fittedHeight
    minimumWidth: fixedWidth
    maximumWidth: fixedWidth
    minimumHeight: fittedHeight
    maximumHeight: fittedHeight

    flags: Qt.Dialog | Qt.WindowStaysOnTopHint | Qt.WindowCloseButtonHint

    onClosing: close => {
        close.accepted = false;
        selectionState.cancel();
    }

    Shortcut {
        sequence: "Escape"
        onActivated: selectionState.cancel()
    }

    SelectionState {
        id: selectionState
        Component.onCompleted: start()
    }

    ColumnLayout {
        id: content
        anchors.fill: parent
        anchors.margins: root.margin
        spacing: Kirigami.Units.largeSpacing

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: Math.max(0, root.pages.indexOf(selectionState.page))

            SelectView {
                selection: selectionState
            }
            WaitingView {}
            FormView {
                id: formView
                selection: selectionState
            }
            FingerprintView {
                selection: selectionState
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        DialogFooter {
            Layout.fillWidth: true
            selection: selectionState
            showAuthorize: selectionState.page === "form"
            authorizeEnabled: selectionState.hasField && !selectionState.fieldLocked
            onAuthorize: formView.submit()
        }
    }
}