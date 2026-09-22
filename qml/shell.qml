import QtQuick
import Quickshell
import Quickshell.Io
import "."

ShellRoot {
    id: shellRoot

    FloatingWindow {
        id: win
        title: "PolyCodex - Universal Layout-Preserving Book & PDF Translator"
        implicitWidth: 880
        implicitHeight: 680
        color: Theme.bgBase

        MainWindow {
            id: mainWin
            anchors.fill: parent
        }
    }

    // IPC Arayuzu (CLI veya baska bir surecten kontrol etmek icin)
    IpcHandler {
        target: "ozdil.polycodex"

        function toggle(): bool {
            win.visible = !win.visible;
            return win.visible;
        }

        function openPdf(path: string): string {
            if (!path || path.length === 0) return "Error: empty path";
            mainWin.inputPath = path;
            win.visible = true;
            return "OK";
        }

        function getStatus(): string {
            return JSON.stringify({
                "isTranslating": mainWin.isTranslating,
                "currentPage": mainWin.currentPage,
                "totalPages": mainWin.totalPages,
                "inputPath": mainWin.inputPath
            });
        }
    }
}
