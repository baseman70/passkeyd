import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

// Cancel always; Authorize only while there is something to submit.
RowLayout {
    id: footer

    required property var selection
    property bool showAuthorize: false
    property bool authorizeEnabled: true

    signal authorize

    spacing: Kirigami.Units.smallSpacing

    Item {
        Layout.fillWidth: true
    }

    Controls.Button {
        text: qsTr("Cancel")
        icon.name: "dialog-cancel"
        onClicked: footer.selection.cancel()
    }

    Controls.Button {
        visible: footer.showAuthorize
        enabled: footer.authorizeEnabled
        text: qsTr("Authorize")
        icon.name: "dialog-ok"
        highlighted: true
        onClicked: footer.authorize()
    }
}
