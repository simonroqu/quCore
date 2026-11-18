# Erzeugt src/dev/roqu/qu/Icons.kt aus den qu-core-Icons (16-px-SVG, nur <path>), für alle Android-Apps.
# Aufruf: python3 android/icons.py back home search …  (immer die ganze Liste angeben, die Datei wird neu geschrieben)
import re, sys, pathlib
here = pathlib.Path(__file__).parent
lines = []
for name in sys.argv[1:]:
    svg = (here / f"../data/icons/qu-{name}-symbolic.svg").read_text()
    assert "<circle" not in svg and "<rect" not in svg and "transform" not in svg, name
    paths = [(re.search(r'\bd="([^"]+)"', p).group(1), "evenodd" in p) for p in re.findall(r"<path\b[^>]*>", svg)]
    args = ", ".join(f'"{d}" to {str(eo).lower()}' for d, eo in paths)
    lines.append(f"    val {name} = icon({args})")
(here / "src/dev/roqu/qu/Icons.kt").write_text(f"""// Erzeugt von android/icons.py aus data/icons (nicht von Hand ändern).
package dev.roqu.qu

import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser

/** 16-px-Icon aus SVG-Pfaden (Pfad, evenodd). Farbe setzt `Icon(tint = …)`. */
private fun icon(vararg paths: Pair<String, Boolean>): ImageVector {{
    val b = ImageVector.Builder(defaultWidth = androidx.compose.ui.unit.Dp(16f), defaultHeight = androidx.compose.ui.unit.Dp(16f),
        viewportWidth = 16f, viewportHeight = 16f)
    for ((d, evenOdd) in paths) {{
        b.addPath(PathParser().parsePathString(d).toNodes(), fill = SolidColor(Color.White),
            pathFillType = if (evenOdd) PathFillType.EvenOdd else PathFillType.NonZero)
    }}
    return b.build()
}}

object QuIcons {{
{chr(10).join(lines)}
}}
""")
print("Icons.kt:", ", ".join(sys.argv[1:]))
