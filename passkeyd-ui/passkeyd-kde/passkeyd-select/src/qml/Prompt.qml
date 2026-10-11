import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// The layout of KDE's own authentication dialogs: a large icon on the left,
// a heading and the content on the right. Children go below the heading.
RowLayout {
    id: prompt

    property string iconName
    property string heading
    default property alias content: body.data

    spacing: Kirigami.Units.largeSpacing

    Kirigami.Icon {
        source: prompt.iconName
        Layout.alignment: Qt.AlignTop
        Layout.preferredWidth: Kirigami.Units.iconSizes.large
        Layout.preferredHeight: Kirigami.Units.iconSizes.large
    }

    ColumnLayout {
        id: body
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: Kirigami.Units.smallSpacing * 2

        Kirigami.Heading {
            text: prompt.heading
            visible: text.length > 0
            level: 4
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }
}