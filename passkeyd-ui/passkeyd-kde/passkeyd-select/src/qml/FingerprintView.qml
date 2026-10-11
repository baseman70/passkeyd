import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Nothing to type: the core drives the sensor and tells us when to retry.
Prompt {
    id: view

    required property var selection

    iconName: "fingerprint"
    heading: view.selection.description

    NoticeMessage {
        selection: view.selection
    }

}