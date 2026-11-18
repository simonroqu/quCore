// Erzeugt von android/icons.py aus data/icons (nicht von Hand ändern).
package dev.roqu.qu

import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser

/** 16-px-Icon aus SVG-Pfaden (Pfad, evenodd). Farbe setzt `Icon(tint = …)`. */
private fun icon(vararg paths: Pair<String, Boolean>): ImageVector {
    val b = ImageVector.Builder(defaultWidth = androidx.compose.ui.unit.Dp(16f), defaultHeight = androidx.compose.ui.unit.Dp(16f),
        viewportWidth = 16f, viewportHeight = 16f)
    for ((d, evenOdd) in paths) {
        b.addPath(PathParser().parsePathString(d).toNodes(), fill = SolidColor(Color.White),
            pathFillType = if (evenOdd) PathFillType.EvenOdd else PathFillType.NonZero)
    }
    return b.build()
}

object QuIcons {
    val back = icon("M10.4 2.4 4.8 8l5.6 5.6 1.1-1.1L6.9 8l4.6-4.5z" to false)
    val home = icon("M8 1.6 15 8h-1.6v6.4H2.6V8H1zm0 2.1L4.1 7.3v5.6h7.8V7.3z" to true)
    val search = icon("M6.8 1.5a5.3 5.3 0 1 0 3.2 9.5l3.6 3.6 1-1-3.6-3.6A5.3 5.3 0 0 0 6.8 1.5zm0 1.3a4 4 0 1 1 0 8 4 4 0 0 1 0-8z" to true)
    val watchlist = icon("M4 1.5h8a.5.5 0 0 1 .5.5v12.5L8 11.6l-4.5 2.9V2a.5.5 0 0 1 .5-.5zm1.1 1.6v8.4L8 9.7l2.9 1.8V3.1z" to true)
    val watchlisted = icon("M4 1.5h8a.5.5 0 0 1 .5.5v12.5L8 11.6l-4.5 2.9V2a.5.5 0 0 1 .5-.5z" to false)
    val later = icon("M4 1.5h8a.5.5 0 0 1 .5.5v12.5L8 11.6l-4.5 2.9V2a.5.5 0 0 1 .5-.5zm1.1 1.6v8.4L8 9.7l2.9 1.8V3.1z" to true)
    val history = icon("M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zm0 1.6a5.4 5.4 0 1 1 0 10.8A5.4 5.4 0 0 1 8 2.6z" to true, "M7.2 4.5h1.6v3.2l2.3 2.3-1.1 1.1-2.8-2.8z" to false)
    val play = icon("M4.5 2.6v10.8L13 8z" to false)
    val subscribe = icon("M4 2.5h8A3.5 3.5 0 0 1 15.5 6v4a3.5 3.5 0 0 1-3.5 3.5H4A3.5 3.5 0 0 1 .5 10V6A3.5 3.5 0 0 1 4 2.5zM4 4a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2zm2.5 1.6L10.5 8l-4 2.4z" to true)
    val subscribed = icon("M4 2.5h8A3.5 3.5 0 0 1 15.5 6v4a3.5 3.5 0 0 1-3.5 3.5H4A3.5 3.5 0 0 1 .5 10V6A3.5 3.5 0 0 1 4 2.5zm2.5 3.1v4.8L10.5 8z" to true)
    val account = icon("M8 1.5a3.25 3.25 0 1 0 0 6.5 3.25 3.25 0 0 0 0-6.5zm0 1.5a1.75 1.75 0 1 1 0 3.5A1.75 1.75 0 0 1 8 3zM8 9c-3 0-5.5 1.6-5.5 4v1.5h11V13c0-2.4-2.5-4-5.5-4zm0 1.5c2.2 0 3.8 1.1 4 2.5H4c.2-1.4 1.8-2.5 4-2.5z" to false)
    val menu = icon("M2 3.3h12v1.4H2zm0 4h12v1.4H2zm0 4h12v1.4H2z" to false)
    val tv = icon("M2.5 2.5h11a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1zm.3 1.3v6.4h10.4V3.8zM5 13h6v1.2H5z" to true)
}
