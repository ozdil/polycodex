pragma Singleton
import QtQuick

QtObject {
    id: root

    // Renk Paleti (Koyu ve Modern Teknik Tema)
    readonly property color bgDark: "#13141c"
    readonly property color bgBase: "#16161e"
    readonly property color bgSurface: "#1a1b26"
    readonly property color bgCard: "#202330"
    readonly property color bgCardHover: "#282c3f"
    readonly property color border: "#2f354a"
    readonly property color borderLight: "#414868"

    readonly property color textMain: "#c0caf5"
    readonly property color textMuted: "#7982a9"
    readonly property color textDim: "#545c7e"

    readonly property color accent: "#7aa2f7"
    readonly property color accentHover: "#89b4fa"
    readonly property color accentCyan: "#7dcfff"
    readonly property color accentGreen: "#9ece6a"
    readonly property color accentOrange: "#ff9e64"
    readonly property color accentRed: "#f7768e"

    // Yaricap ve Bosluklar
    readonly property int radiusSm: 6
    readonly property int radiusMd: 10
    readonly property int radiusLg: 14

    // Zorunlu Tipografi Standardi
    readonly property string fontFamily: "JetBrainsMono Nerd Font, JetBrains Mono, monospace"
    readonly property string monoFont: "JetBrainsMono Nerd Font, JetBrains Mono, monospace"
}
