import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

// One text prompt: the password form, or a single step of a PAM conversation.
// The core decides what is asked and whether the answer was right.
Prompt {
    id: view

    required property var selection

    iconName: "dialog-password"
    heading: view.selection.description

    function submit() {
        if (!selection.hasField || selection.fieldLocked) {
            return;
        }
        const answer = input.text;
        input.text = "";
        selection.submit(answer);
    }

    Controls.TextField {
        id: input

        Layout.fillWidth: true
        // Never let the layout squeeze the field.
        Layout.minimumHeight: implicitHeight
        Layout.preferredHeight: implicitHeight

        visible: view.selection.hasField
        // Locked while the core checks the last answer.
        enabled: !view.selection.fieldLocked
        echoMode: view.selection.fieldSecure ? TextInput.Password : TextInput.Normal
        placeholderText: view.selection.fieldPlaceholder

        onAccepted: view.submit()

        onVisibleChanged: {
            if (visible && enabled) {
                forceActiveFocus();
            }
        }
        onEnabledChanged: {
            if (enabled && visible) {
                forceActiveFocus();
            }
        }
    }

    NoticeMessage {
        selection: view.selection
    }

}