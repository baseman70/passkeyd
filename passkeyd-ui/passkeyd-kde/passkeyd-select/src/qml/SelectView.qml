import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

// Account picker: choosing a row hands the decision to the core.
Prompt {
    id: view

    required property var selection

    iconName: "user-identity"
    heading: view.selection.description

    Controls.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true
        // About three rows; the list scrolls beyond that.
        Layout.preferredHeight: Kirigami.Units.gridUnit * 6

        // The framed list look KDE's own lists use.
        Component.onCompleted: background.visible = true

        ListView {
            clip: true
            model: view.selection.accounts

            delegate: Controls.ItemDelegate {
                required property int index
                required property string modelData

                width: ListView.view.width
                text: modelData
                icon.name: "user-identity"

                onClicked: view.selection.choose(index)
            }
        }
    }
}