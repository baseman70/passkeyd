import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls

// Choice sent; the core will either pick an auth method or close the window.
Item {
    implicitHeight: spinner.implicitHeight

    Controls.BusyIndicator {
        id: spinner
        anchors.centerIn: parent
        running: true
    }
}