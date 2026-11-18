// Gemeinsame Android-Oberfläche der Qu-Apps (Compose), so wie qu-core sie am Desktop baut: Theme „Last Horizon“,
// schwebende Nav-Bubble oben links, Suche wie Spotlight, Poster-Raster, Tabs, Kacheln. Jede App bindet diesen Ordner
// über `cargo metadata` ein (gleicher qu-core-Stand wie der Rust-Teil). Texte der Oberfläche: Englisch.
// Android TV: alles per Fernbedienung erreichbar – jedes klickbare Element bekommt `tvFocus()` vor `clickable`.
package dev.roqu.qu

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import coil3.compose.AsyncImage

// Farben wie qu-core ohne Omarchy (theme.rs `DEFAULT_COLORS`, „Last Horizon“) und die Glas-Bubbles aus style.css.
val Bg = Color(0xFF0C0B0C)
val Fg = Color(0xFFFAFCFB)
val Accent = Color(0xFFB59790)
val Red = Color(0xFFC38B7B)
val Dim = Fg.copy(alpha = 0.65f)
val TileBg = Fg.copy(alpha = 0.06f)
val Glass = Color(0x8C18181C)
val GlassBorder = Color(0x1FFFFFFF)
val SearchCard = Color(0xEB1E1E22)

/** Platz unter der Nav-Bubble, damit sie nichts verdeckt. */
val TopSpace = 64.dp

@Composable
fun QuTheme(content: @Composable () -> Unit) {
    val scheme = darkColorScheme(
        primary = Accent, onPrimary = Bg, background = Bg, onBackground = Fg, surface = Bg, onSurface = Fg,
        surfaceVariant = TileBg, onSurfaceVariant = Dim, error = Red,
    )
    MaterialTheme(colorScheme = scheme) { Surface(Modifier.fillMaxSize(), color = Bg, content = content) }
}

/** Fokus sichtbar machen (Android TV, Fernbedienung): weißer Rahmen, Karten leicht größer. Vor `clickable` setzen.
 *  Breite Zeilen mit `grow = 1f`, sonst ragen sie über den Rand. */
@Composable
fun Modifier.tvFocus(shape: Shape = RoundedCornerShape(10.dp), grow: Float = 1.05f): Modifier {
    var focused by remember { mutableStateOf(false) }
    val scale by animateFloatAsState(if (focused) grow else 1f, label = "focus")
    return onFocusChanged { focused = it.isFocused }
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .then(if (focused) Modifier.border(3.dp, Color.White, shape) else Modifier)
}

/** Dunkles Glas wie `.bubble` (qu-core style.css). */
fun Modifier.glass(shape: Shape = CircleShape) =
    shadow(12.dp, shape).background(Glass, shape).border(1.dp, GlassBorder, shape)

/** Runder Icon-Knopf (Bubble, Aktionen). `tint` z. B. Akzent, wenn aktiv. */
@Composable
fun IconButton(icon: ImageVector, label: String, tint: Color = Color.White, size: Dp = 40.dp, onClick: () -> Unit) {
    Box(
        Modifier.size(size).clip(CircleShape).tvFocus(CircleShape, 1.1f).clickable(onClickLabel = label, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, contentDescription = label, tint = tint, modifier = Modifier.size(size * 0.45f)) }
}

/** Seitenstapel wie `qu_core::nav` (Zurück/Home). Seiten sind Datenklassen der App. */
class Nav<P : Any>(start: P) {
    val stack = mutableStateListOf(start)
    val page: P get() = stack.last()
    fun open(p: P) { stack.add(p) }
    fun back() { if (stack.size > 1) stack.removeAt(stack.lastIndex) }
    fun home() { stack.removeRange(1, stack.size) }
    /** Aktuelle Seite tauschen (z. B. nächste Episode), ohne den Verlauf zu verlängern. */
    fun replace(p: P) { stack[stack.lastIndex] = p }
}

/**
 * Rahmen jeder Qu-App: Seite (frisch je Eintrag im Stapel), darüber die Nav-Bubble (Zurück, Home, Suche und
 * `buttons` der App, wie `[nav] top`), Suche wie Spotlight mit Verlauf. `bubble(page)` = false z. B. im Player.
 */
@Composable
fun <P : Any> QuApp(
    nav: Nav<P>,
    placeholder: String,
    searches: () -> List<String>,
    onSearch: (String) -> Unit,
    bubble: (P) -> Boolean = { true },
    buttons: @Composable RowScope.() -> Unit = {},
    content: @Composable (P) -> Unit,
) {
    var searching by remember { mutableStateOf(false) }
    BackHandler(nav.stack.size > 1) { nav.back() }
    val page = nav.page
    val inner = remember { FocusRequester() }
    Box(Modifier.fillMaxSize()) {
        // key: jede Seite frisch (eigener Zustand, Player wird beim Verlassen freigegeben).
        key(page) { Box(Modifier.focusRequester(inner).focusGroup()) { content(page) } }
        if (bubble(page)) {
            // Runter: die schwebende Bubble liegt über der Seite, Compose findet von hier aus keinen Weg hinein.
            Row(
                Modifier.safeDrawingPadding().padding(12.dp).glass().padding(3.dp).onPreviewKeyEvent {
                    val down = it.type == KeyEventType.KeyDown && it.key == Key.DirectionDown
                    down && inner.requestFocus(FocusDirection.Down)
                },
            ) {
                if (nav.stack.size > 1) IconButton(QuIcons.back, "Back") { nav.back() }
                IconButton(QuIcons.home, "Home") { nav.home() }
                IconButton(QuIcons.search, "Search") { searching = true }
                buttons()
            }
        }
    }
    if (searching) {
        SearchModal(placeholder, searches, close = { searching = false }) { q ->
            searching = false
            onSearch(q)
        }
    }
}

/** Suchfenster wie Spotlight (qu-core `search.rs`): abgedunkelt, Karte oben in der Mitte, letzte Suchen darunter. */
@Composable
fun SearchModal(placeholder: String, searches: () -> List<String>, close: () -> Unit, submit: (String) -> Unit) {
    var query by remember { mutableStateOf("") }
    val recent = remember { searches() }
    val field = remember { FocusRequester() }
    LaunchedEffect(Unit) { field.requestFocus() }
    Dialog(onDismissRequest = close, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        // Abdunkeln macht die Box unten (wie am Desktop 45 %), nicht zusätzlich Android.
        (LocalView.current.parent as? DialogWindowProvider)?.window?.setDimAmount(0f)
        val focus = LocalFocusManager.current
        Box(
            Modifier.fillMaxSize().background(Color(0x73000000))
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = close),
            contentAlignment = Alignment.TopCenter,
        ) {
            val shape = RoundedCornerShape(18.dp)
            Column(
                Modifier.padding(top = 72.dp, start = 16.dp, end = 16.dp).widthIn(max = 560.dp).fillMaxWidth()
                    .shadow(30.dp, shape).background(SearchCard, shape).border(1.dp, GlassBorder, shape)
                    .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {}
                    .padding(8.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 8.dp)) {
                    Icon(QuIcons.search, null, tint = Dim, modifier = Modifier.size(18.dp))
                    BasicTextField(
                        value = query,
                        onValueChange = { query = it },
                        singleLine = true,
                        textStyle = TextStyle(color = Color.White, fontSize = 19.sp),
                        cursorBrush = SolidColor(Accent),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                        keyboardActions = KeyboardActions(onSearch = { if (query.isNotBlank()) submit(query.trim()) }),
                        // Fernbedienung: Runter verlässt das Feld zu den letzten Suchen (im Textfeld bewegt es sonst nur den Cursor).
                        modifier = Modifier.weight(1f).padding(start = 12.dp).heightIn(min = 44.dp).focusRequester(field)
                            .onPreviewKeyEvent {
                                val down = it.type == KeyEventType.KeyDown && it.key == Key.DirectionDown
                                down && focus.moveFocus(FocusDirection.Down)
                            },
                        decorationBox = { inner ->
                            Box(contentAlignment = Alignment.CenterStart) {
                                if (query.isEmpty()) Text(placeholder, color = Dim, fontSize = 19.sp)
                                inner()
                            }
                        },
                    )
                }
                // Leeres Feld = letzte Suchen, sonst die passenden davon (`[search] suggestions = "history"`).
                recent.filter { it.contains(query.trim(), ignoreCase = true) }.forEach { s ->
                    Text(
                        s,
                        color = Color.White,
                        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).tvFocus(RoundedCornerShape(12.dp), 1f)
                            .clickable { submit(s) }.padding(horizontal = 12.dp, vertical = 10.dp),
                    )
                }
            }
        }
    }
}

/** Karte im Raster: Bild (`ratio` = Breite/Höhe, Poster 2:3, Videos 16:9), fetter Titel, Zusatzzeile, Fortschritt. */
@Composable
fun PosterCard(
    title: String,
    image: String?,
    ratio: Float = 2f / 3f,
    label: String? = null,
    fraction: Double? = null,
    onClick: () -> Unit,
) {
    Column(Modifier.tvFocus().clip(RoundedCornerShape(10.dp)).clickable(onClick = onClick).padding(4.dp)) {
        Box {
            AsyncImage(
                model = image,
                contentDescription = title,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxWidth().aspectRatio(ratio).clip(RoundedCornerShape(8.dp)).background(TileBg),
            )
            if (fraction != null) {
                LinearProgressIndicator(
                    progress = { fraction.toFloat() },
                    color = Accent,
                    trackColor = Color(0x66000000),
                    modifier = Modifier.fillMaxWidth().align(Alignment.BottomCenter).padding(6.dp).clip(CircleShape),
                )
            }
        }
        Text(title, maxLines = 2, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.Bold, fontSize = 15.sp,
            color = Color.White, modifier = Modifier.padding(top = 6.dp))
        if (label != null) Text(label, fontSize = 13.sp, color = Dim, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** Graue Platzhalter in Form der Karten, solange nichts da ist (wie `.skeleton` am Desktop). */
fun LazyGridScope.ghosts(n: Int, ratio: Float = 2f / 3f) {
    items(n) {
        Column(Modifier.padding(4.dp)) {
            Box(Modifier.fillMaxWidth().aspectRatio(ratio).clip(RoundedCornerShape(8.dp)).background(TileBg))
            Box(Modifier.padding(top = 8.dp).fillMaxWidth(0.7f).height(14.dp).clip(RoundedCornerShape(4.dp)).background(TileBg))
        }
    }
}

/** Abschnitts-Überschrift über die ganze Breite (erste mit `top = 0.dp`). */
fun LazyGridScope.header(text: String, top: Dp = 20.dp) {
    item(span = { GridItemSpan(maxLineSpan) }) {
        Text(text, fontWeight = FontWeight.Bold, fontSize = 24.sp, color = Color.White, modifier = Modifier.padding(top = top, bottom = 4.dp))
    }
}

/** Hinweis über die ganze Breite (Fehler in `Red`). */
fun LazyGridScope.note(text: String, color: Color = Dim) {
    item(span = { GridItemSpan(maxLineSpan) }) { Text(text, color = color, modifier = Modifier.padding(vertical = 8.dp)) }
}

/** Element über die ganze Breite (z. B. Kopf der Detailseite). */
fun LazyGridScope.wide(content: @Composable () -> Unit) {
    item(span = { GridItemSpan(maxLineSpan) }) { content() }
}

/** Seite als Raster unter der Nav-Bubble. `minWidth`: Karten so breit (Poster 120 dp, Videos 260 dp). */
@Composable
fun Grid(minWidth: Dp = 120.dp, content: LazyGridScope.() -> Unit) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(minWidth),
        contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = TopSpace, bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.safeDrawingPadding(),
        content = content,
    )
}

/** Umschalter wie `.tabs` am Desktop (Sub/Dub, Staffeln): gewählt = Akzentfarbe mit Strich darunter. */
@Composable
fun TabButton(text: String, selected: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    Column(
        Modifier.alpha(if (enabled) 1f else 0.4f).clip(RoundedCornerShape(8.dp)).tvFocus(RoundedCornerShape(8.dp))
            .clickable(enabled = enabled, onClick = onClick).padding(horizontal = 14.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(text, color = if (selected) Accent else Fg, fontWeight = FontWeight.Bold)
        Box(Modifier.padding(top = 4.dp).width(28.dp).height(2.dp).background(if (selected) Accent else Color.Transparent))
    }
}

/** Kachel wie `.episode` am Desktop: fetter Titel, gedimmte Zeile, Fortschritt; gesehen = halb durchsichtig. */
@Composable
fun Tile(title: String, subtitle: String, fraction: Double?, seen: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    val shape = RoundedCornerShape(12.dp)
    Column(
        Modifier.alpha(if (!enabled) 0.35f else if (seen) 0.5f else 1f).tvFocus(shape)
            .clip(shape).background(TileBg).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 12.dp, vertical = 10.dp),
    ) {
        Text(title, fontWeight = FontWeight.Bold, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(subtitle, color = Dim, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        LinearProgressIndicator(
            progress = { fraction?.toFloat() ?: 0f },
            color = Accent,
            trackColor = if (fraction != null) Fg.copy(alpha = 0.15f) else Color.Transparent,
            modifier = Modifier.fillMaxWidth().padding(top = 6.dp).height(3.dp).clip(CircleShape),
        )
    }
}

/** Platzhalter-Kacheln, solange die Liste lädt. */
fun LazyGridScope.ghostTiles(n: Int) {
    items(n) { Box(Modifier.height(66.dp).clip(RoundedCornerShape(12.dp)).background(TileBg)) }
}

/**
 * Kopf einer Detailseite wie am Desktop (`show.rs`): Bild links, großer Titel, Infozeile, Beschreibung mit
 * „Show more“, darunter `actions` (Tabs, Lesezeichen …).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DetailHeader(
    title: String,
    image: String?,
    ratio: Float = 2f / 3f,
    meta: String? = null,
    description: String? = null,
    actions: @Composable RowScope.() -> Unit = {},
) {
    var more by remember { mutableStateOf(false) }
    Row(horizontalArrangement = Arrangement.spacedBy(20.dp), modifier = Modifier.padding(bottom = 12.dp)) {
        AsyncImage(
            model = image,
            contentDescription = null,
            contentScale = ContentScale.Crop,
            modifier = Modifier.width(if (ratio < 1f) 140.dp else 260.dp).aspectRatio(ratio).clip(RoundedCornerShape(10.dp)).background(TileBg),
        )
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(title, fontWeight = FontWeight.Bold, fontSize = 28.sp, lineHeight = 32.sp, color = Color.White)
            if (!meta.isNullOrEmpty()) Text(meta, color = Dim, fontSize = 14.sp)
            if (!description.isNullOrEmpty()) {
                Text(description, maxLines = if (more) Int.MAX_VALUE else 4, overflow = TextOverflow.Ellipsis, fontSize = 15.sp)
                TextButton(onClick = { more = !more }, modifier = Modifier.tvFocus(RoundedCornerShape(50))) {
                    Text(if (more) "Show less" else "Show more", color = Fg)
                }
            }
            // Umbrechen statt quetschen: viele Sprach-Tabs (BoxQu) schieben sonst das Lesezeichen aus dem Bild.
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) { actions() }
        }
    }
}
