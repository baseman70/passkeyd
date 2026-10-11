import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// A hint or an error from the core, as Plasma's inline message banner.
Kirigami.InlineMessage {
    required property var selection

    Layout.fillWidth: true
    visible: text.length > 0
    text: selection.notice
    type: selection.noticeIsError ? Kirigami.MessageType.Error : Kirigami.MessageType.Information
}
