use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    unsafe {
        CxxQtBuilder::new_qml_module(
            QmlModule::new("passkeyd.ui.select")
                .qml_file("src/qml/Main.qml")
                .qml_file("src/qml/Prompt.qml")
                .qml_file("src/qml/NoticeMessage.qml")
                .qml_file("src/qml/DialogFooter.qml")
                .qml_file("src/qml/SelectView.qml")
                .qml_file("src/qml/FormView.qml")
                .qml_file("src/qml/FingerprintView.qml")
                .qml_file("src/qml/WaitingView.qml"),
        )
        .file("src/state.rs")
        .cc_builder(|cc| {
            cc.flag("-Wno-sfinae-incomplete");
        })
        .build();
    }
}
