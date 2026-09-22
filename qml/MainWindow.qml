import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import "."

Rectangle {
    id: root
    color: Theme.bgBase

    // State Variables (Default Application Language: English)
    property string inputPath: ""
    property string sourceLang: "auto"
    property string targetLang: "en"
    property string customOutputName: ""
    property string customOutputDir: ""
    property string selectedProvider: "mock" // mock, libretranslate, ollama

    property bool isTranslating: false
    property int currentPage: 0
    property int totalPages: 0
    property real progressPercentage: 0.0
    property string statusMessage: "Ready. Please select a PDF document to translate."
    property var logLines: []

    // Dynamic Default Output Calculations
    readonly property string defaultStem: {
        if (!inputPath || inputPath.length === 0) return "document";
        var parts = inputPath.split("/");
        var fileName = parts[parts.length - 1];
        var dotIndex = fileName.lastIndexOf(".");
        return dotIndex > 0 ? fileName.substring(0, dotIndex) : fileName;
    }

    readonly property string resolvedOutputName: {
        if (customOutputName.trim().length > 0) {
            var name = customOutputName.trim();
            return name.toLowerCase().endsWith(".pdf") ? name : (name + ".pdf");
        }
        return defaultStem + "_" + targetLang + ".pdf";
    }

    readonly property string resolvedOutputDir: {
        if (customOutputDir.trim().length > 0) return customOutputDir.trim();
        if (!inputPath || inputPath.length === 0) return "/home/ozdil/Documents";
        var lastSlash = inputPath.lastIndexOf("/");
        return lastSlash > 0 ? inputPath.substring(0, lastSlash) : ".";
    }

    // External Rust Engine Process (PolyCodex Core)
    Process {
        id: translatorProc
        command: [
            "polycodex",
            "--input", root.inputPath,
            "--source-lang", root.sourceLang,
            "--target-lang", root.targetLang,
            "--output-name", root.resolvedOutputName,
            "--output-dir", root.resolvedOutputDir,
            "--provider", root.selectedProvider,
            "--non-interactive"
        ]

        stdout: SplitParser {
            onRead: text => {
                root.appendLog(text);
                // Progress matcher: Sayfa {pos}/{len} (%{percent})
                var match = text.match(/Sayfa (\d+)\/(\d+) \(%([\d\.]+)\)/);
                if (match) {
                    root.currentPage = parseInt(match[1]);
                    root.totalPages = parseInt(match[2]);
                    root.progressPercentage = parseFloat(match[3]) / 100.0;
                    root.statusMessage = "Processing: Page " + root.currentPage + " / " + root.totalPages;
                }
            }
        }

        stderr: SplitParser {
            onRead: text => {
                root.appendLog("[ERROR] " + text);
                if (text.indexOf("HATA: Halen devam etmekte olan") !== -1 || text.indexOf("Already running") !== -1) {
                    root.statusMessage = "Concurrency Lock: Another translation process is already in progress.";
                }
            }
        }

        onExited: exitCode => {
            root.isTranslating = false;
            if (exitCode === 0) {
                root.progressPercentage = 1.0;
                root.statusMessage = "Completed: " + root.resolvedOutputDir + "/" + root.resolvedOutputName;
                root.appendLog("[SYSTEM] Translation finished successfully with 1-1 layout preservation.");
            } else {
                root.appendLog("[SYSTEM] Process exited with error code: " + exitCode);
            }
        }
    }

    // Zenity File Picker Process (Browse...)
    Process {
        id: pickInputProc
        command: ["zenity", "--file-selection", "--title=Select PDF Document to Translate", "--file-filter=PDF Documents (*.pdf) | *.pdf *.PDF"]
        stdout: SplitParser {
            onRead: text => {
                var p = String(text || "").trim();
                if (p.length > 0) {
                    root.inputPath = p;
                    root.appendLog("[FILE] Selected: " + p);
                }
            }
        }
    }

    // Zenity Directory Picker Process (Browse Directory...)
    Process {
        id: pickDirProc
        command: ["zenity", "--file-selection", "--directory", "--title=Select Output Directory"]
        stdout: SplitParser {
            onRead: text => {
                var p = String(text || "").trim();
                if (p.length > 0) {
                    root.customOutputDir = p;
                    root.appendLog("[DIR] Selected: " + p);
                }
            }
        }
    }

    function appendLog(line) {
        var lines = root.logLines.slice();
        lines.push(line);
        if (lines.length > 200) lines.shift();
        root.logLines = lines;
    }

    function startTranslation() {
        if (!inputPath || inputPath.trim().length === 0) {
            statusMessage = "Warning: Please select a valid PDF file first.";
            return;
        }
        root.isTranslating = true;
        root.currentPage = 0;
        root.progressPercentage = 0.0;
        root.statusMessage = "Translation engine starting...";
        root.appendLog("[SYSTEM] Engine launched: " + root.inputPath + " -> " + root.targetLang);
        translatorProc.running = true;
    }

    // Main Layout
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 16

        // 1. Header Area
        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            ColumnLayout {
                spacing: 4
                Text {
                    text: "POLYCODEX"
                    font.family: Theme.fontFamily
                    font.pixelSize: 22
                    font.bold: true
                    color: Theme.textMain
                }
                Text {
                    text: "Universal Layout-Preserving Multi-Lingual Book & PDF Engine (5000 Pages)"
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    color: Theme.textMuted
                }
            }

            Item { Layout.fillWidth: true }

            // Concurrency Lock Badge
            Rectangle {
                implicitWidth: 140
                implicitHeight: 32
                radius: Theme.radiusSm
                color: root.isTranslating ? Theme.bgCardHover : Theme.bgSurface
                border.color: root.isTranslating ? Theme.accentOrange : Theme.accentGreen
                border.width: 1

                Text {
                    anchors.centerIn: parent
                    text: root.isTranslating ? "[LOCKED]" : "[READY]"
                    font.family: Theme.monoFont
                    font.pixelSize: 11
                    font.bold: true
                    color: root.isTranslating ? Theme.accentOrange : Theme.accentGreen
                }
            }
        }

        // 2. Input Document Card
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 92
            color: Theme.bgSurface
            radius: Theme.radiusMd
            border.color: Theme.border
            border.width: 1

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 8

                Text {
                    text: "Input PDF Document (Drag & drop or enter path)"
                    font.family: Theme.fontFamily
                    font.pixelSize: 12
                    font.bold: true
                    color: Theme.textMuted
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: 36
                        color: Theme.bgCard
                        radius: Theme.radiusSm
                        border.color: Theme.borderLight

                        TextInput {
                            anchors.fill: parent
                            anchors.margins: 8
                            text: root.inputPath
                            font.family: Theme.monoFont
                            font.pixelSize: 12
                            color: Theme.textMain
                            selectByMouse: true
                            onTextChanged: root.inputPath = text
                        }
                    }

                    // Browse Button
                    Rectangle {
                        implicitWidth: 95
                        implicitHeight: 36
                        radius: Theme.radiusSm
                        color: browseBtnMouse.containsMouse ? Theme.bgCardHover : Theme.bgCard
                        border.color: Theme.borderLight
                        border.width: 1

                        Text {
                            anchors.centerIn: parent
                            text: "Browse..."
                            font.family: Theme.fontFamily
                            font.pixelSize: 12
                            font.bold: true
                            color: Theme.accent
                        }

                        MouseArea {
                            id: browseBtnMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: pickInputProc.running = true
                        }
                    }
                }
            }
        }

        // 3. Configuration & Language Selection Grid
        RowLayout {
            Layout.fillWidth: true
            spacing: 16

            // Left Card: Languages & Provider
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                implicitHeight: 190
                color: Theme.bgSurface
                radius: Theme.radiusMd
                border.color: Theme.border
                border.width: 1

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 14
                    spacing: 10

                    Text {
                        text: "Translation Settings"
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.bold: true
                        color: Theme.textMuted
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        // Source Language Dropdown
                        ColumnLayout {
                            Layout.fillWidth: true
                            Text {
                                text: "Source Language:"
                                font.family: Theme.fontFamily
                                font.pixelSize: 11
                                color: Theme.textDim
                            }
                            ComboBox {
                                id: srcLangCombo
                                Layout.fillWidth: true
                                implicitHeight: 32
                                model: [
                                    { name: "Auto Detect (auto)", code: "auto" },
                                    { name: "English (en)", code: "en" },
                                    { name: "Turkish / Türkçe (tr)", code: "tr" },
                                    { name: "German / Deutsch (de)", code: "de" },
                                    { name: "French / Français (fr)", code: "fr" },
                                    { name: "Spanish / Español (es)", code: "es" },
                                    { name: "Italian / Italiano (it)", code: "it" },
                                    { name: "Russian / Русский (ru)", code: "ru" },
                                    { name: "Arabic / العربية (ar)", code: "ar" },
                                    { name: "Chinese / 中文 (zh)", code: "zh" },
                                    { name: "Japanese / 日本語 (ja)", code: "ja" },
                                    { name: "Korean / 한국어 (ko)", code: "ko" },
                                    { name: "Portuguese / Português (pt)", code: "pt" }
                                ]
                                textRole: "name"
                                currentIndex: 0
                                onCurrentIndexChanged: {
                                    root.sourceLang = model[currentIndex].code;
                                }
                                background: Rectangle {
                                    color: Theme.bgCard
                                    radius: Theme.radiusSm
                                    border.color: Theme.borderLight
                                }
                                contentItem: Text {
                                    text: srcLangCombo.displayText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    color: Theme.textMain
                                    verticalAlignment: Text.AlignVCenter
                                    leftPadding: 8
                                }
                            }
                        }

                        // Target Language Dropdown (Default: English)
                        ColumnLayout {
                            Layout.fillWidth: true
                            Text {
                                text: "Target Language:"
                                font.family: Theme.fontFamily
                                font.pixelSize: 11
                                color: Theme.textDim
                            }
                            ComboBox {
                                id: dstLangCombo
                                Layout.fillWidth: true
                                implicitHeight: 32
                                model: [
                                    { name: "English (en)", code: "en" },
                                    { name: "Turkish / Türkçe (tr)", code: "tr" },
                                    { name: "German / Deutsch (de)", code: "de" },
                                    { name: "French / Français (fr)", code: "fr" },
                                    { name: "Spanish / Español (es)", code: "es" },
                                    { name: "Italian / Italiano (it)", code: "it" },
                                    { name: "Russian / Русский (ru)", code: "ru" },
                                    { name: "Arabic / العربية (ar)", code: "ar" },
                                    { name: "Chinese / 中文 (zh)", code: "zh" },
                                    { name: "Japanese / 日本語 (ja)", code: "ja" },
                                    { name: "Korean / 한국어 (ko)", code: "ko" },
                                    { name: "Portuguese / Português (pt)", code: "pt" },
                                    { name: "Dutch / Nederlands (nl)", code: "nl" },
                                    { name: "Polish / Polski (pl)", code: "pl" },
                                    { name: "Swedish / Svenska (sv)", code: "sv" },
                                    { name: "Ukrainian / Українська (uk)", code: "uk" },
                                    { name: "Greek / Ελληνικά (el)", code: "el" },
                                    { name: "Hindi / हिन्दी (hi)", code: "hi" }
                                ]
                                textRole: "name"
                                currentIndex: 0 // Default English (en)
                                onCurrentIndexChanged: {
                                    root.targetLang = model[currentIndex].code;
                                }
                                background: Rectangle {
                                    color: Theme.bgCard
                                    radius: Theme.radiusSm
                                    border.color: Theme.borderLight
                                }
                                contentItem: Text {
                                    text: dstLangCombo.displayText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    font.bold: true
                                    color: Theme.accentCyan
                                    verticalAlignment: Text.AlignVCenter
                                    leftPadding: 8
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4
                        Text {
                            text: "Translation Provider:"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            color: Theme.textDim
                        }
                        RowLayout {
                            spacing: 8
                            Repeater {
                                model: ["mock", "libretranslate", "ollama"]
                                delegate: Rectangle {
                                    implicitWidth: 100
                                    implicitHeight: 28
                                    radius: Theme.radiusSm
                                    color: root.selectedProvider === modelData ? Theme.accent : Theme.bgCard
                                    Text {
                                        anchors.centerIn: parent
                                        text: modelData.toUpperCase()
                                        font.family: Theme.monoFont
                                        font.pixelSize: 10
                                        font.bold: true
                                        color: root.selectedProvider === modelData ? Theme.bgBase : Theme.textMain
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.selectedProvider = modelData
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Right Card: Output Destination & Defaults
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                implicitHeight: 190
                color: Theme.bgSurface
                radius: Theme.radiusMd
                border.color: Theme.border
                border.width: 1

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 14
                    spacing: 8

                    Text {
                        text: "Output Configuration (Default if unchanged)"
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        font.bold: true
                        color: Theme.textMuted
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Text {
                            text: "Output File Name: [Default: " + root.defaultStem + "_" + root.targetLang + ".pdf]"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            color: Theme.textDim
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            implicitHeight: 32
                            color: Theme.bgCard
                            radius: Theme.radiusSm
                            TextInput {
                                anchors.fill: parent
                                anchors.margins: 6
                                text: root.customOutputName
                                font.family: Theme.monoFont
                                font.pixelSize: 12
                                color: Theme.textMain
                                onTextChanged: root.customOutputName = text
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Text {
                            text: "Output Directory: [Default: " + root.resolvedOutputDir + "]"
                            font.family: Theme.fontFamily
                            font.pixelSize: 11
                            color: Theme.textDim
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Rectangle {
                                Layout.fillWidth: true
                                implicitHeight: 32
                                color: Theme.bgCard
                                radius: Theme.radiusSm
                                TextInput {
                                    anchors.fill: parent
                                    anchors.margins: 6
                                    text: root.customOutputDir
                                    font.family: Theme.monoFont
                                    font.pixelSize: 12
                                    color: Theme.textMain
                                    onTextChanged: root.customOutputDir = text
                                }
                            }
                            Rectangle {
                                implicitWidth: 80
                                implicitHeight: 32
                                radius: Theme.radiusSm
                                color: dirBrowseBtnMouse.containsMouse ? Theme.bgCardHover : Theme.bgCard
                                border.color: Theme.borderLight
                                border.width: 1

                                Text {
                                    anchors.centerIn: parent
                                    text: "Browse..."
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 11
                                    font.bold: true
                                    color: Theme.accent
                                }

                                MouseArea {
                                    id: dirBrowseBtnMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: pickDirProc.running = true
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Action Button & Progress
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 64
            color: Theme.bgSurface
            radius: Theme.radiusMd
            border.color: Theme.border
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 16

                Rectangle {
                    implicitWidth: 170
                    implicitHeight: 40
                    radius: Theme.radiusSm
                    color: root.isTranslating ? Theme.bgCardHover : Theme.accent
                    opacity: root.isTranslating ? 0.6 : 1.0

                    Text {
                        anchors.centerIn: parent
                        text: root.isTranslating ? "Translating..." : "Start Translation"
                        font.family: Theme.fontFamily
                        font.pixelSize: 13
                        font.bold: true
                        color: root.isTranslating ? Theme.textMuted : Theme.bgBase
                    }

                    MouseArea {
                        anchors.fill: parent
                        enabled: !root.isTranslating
                        cursorShape: root.isTranslating ? Qt.ArrowCursor : Qt.PointingHandCursor
                        onClicked: root.startTranslation()
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Text {
                        text: root.statusMessage
                        font.family: Theme.monoFont
                        font.pixelSize: 11
                        color: Theme.textMain
                    }

                    // Progress Bar
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: 8
                        radius: 4
                        color: Theme.bgCard

                        Rectangle {
                            height: parent.height
                            radius: 4
                            width: parent.width * root.progressPercentage
                            color: Theme.accentGreen
                        }
                    }
                }
            }
        }

        // 5. Terminal & Engine Log Console
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: Theme.bgDark
            radius: Theme.radiusMd
            border.color: Theme.border
            border.width: 1

            ListView {
                anchors.fill: parent
                anchors.margins: 12
                clip: true
                model: root.logLines
                delegate: Text {
                    text: modelData
                    font.family: Theme.monoFont
                    font.pixelSize: 11
                    color: modelData.indexOf("[ERROR]") !== -1 || modelData.indexOf("[HATA]") !== -1 ? Theme.accentRed :
                           modelData.indexOf("[SYSTEM]") !== -1 || modelData.indexOf("[SISTEM]") !== -1 ? Theme.accentCyan : Theme.textDim
                }
                onCountChanged: positionViewAtEnd()
            }
        }
    }
}
