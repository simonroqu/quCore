// Gemeinsamer Video-Player der Qu-Apps auf Android (Media3/ExoPlayer, wie `player.rs` am Desktop): Vollbild quer,
// Header (Referer, User-Agent, Cookie), Untertitel als Spuren, getrennte Tonspur (YouTube), Fortsetzen, Fortschritt
// alle 5 s, „Skip …“-Bubble (holt sich auf dem TV den Fokus), „Weiter“-Knopf = nächstes Video der App.
package dev.roqu.qu

import android.app.Activity
import android.content.pm.ActivityInfo
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.source.MergingMediaSource
import androidx.media3.ui.DefaultTimeBar
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.delay
import java.io.File

/** Untertitel: lokaler Pfad oder Adresse; `label` = Name im Menü. */
data class Subtitle(val uri: String, val label: String, val mime: String = MimeTypes.TEXT_VTT)

/** Abschnitt zum Überspringen (Opening, Sponsor …), Sekunden. */
data class Skip(val start: Double, val end: Double, val label: String)

/**
 * Was gespielt wird. `mime`: `MimeTypes.APPLICATION_M3U8` (HLS), `APPLICATION_MPD` (DASH) oder null (erkennt
 * ExoPlayer an der Adresse/Datei). `audio`: eigene Tonspur (YouTube liefert Bild und Ton getrennt).
 * `start` in Sekunden (Fortsetzen), `live` = keine Fortschritts-Speicherung.
 */
data class Video(
    val url: String,
    val mime: String? = null,
    val audio: String? = null,
    val headers: Map<String, String> = emptyMap(),
    val userAgent: String? = null,
    val subtitles: List<Subtitle> = emptyList(),
    val start: Double? = null,
    val speed: Float = 1f,
    val live: Boolean = false,
)

/** Querformat und Vollbild, solange der Player offen ist. */
@Composable
fun Fullscreen() {
    val activity = LocalContext.current as Activity
    DisposableEffect(Unit) {
        val bars = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
        activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        bars.hide(WindowInsetsCompat.Type.systemBars())
        onDispose {
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
            bars.show(WindowInsetsCompat.Type.systemBars())
        }
    }
}

/**
 * Player-Seite. `load` holt das Video (Fehlertext über `message`), `skips` läuft danach (darf Werte aus `load`
 * nutzen). `save(pos, dur)` alle 5 s, am Ende und beim Verlassen. `next` = nächstes Video (Knopf und am Ende);
 * null = kein „Weiter“-Knopf.
 */
@OptIn(UnstableApi::class)
@Composable
fun VideoPlayer(
    load: suspend () -> Video,
    message: (Exception) -> String,
    save: (pos: Double, dur: Double) -> Unit,
    skips: suspend () -> List<Skip> = { emptyList() },
    next: (() -> Unit)? = null,
) {
    val ctx = LocalContext.current
    val player = remember { ExoPlayer.Builder(ctx).build() }
    var segs by remember { mutableStateOf(listOf<Skip>()) }
    var skip by remember { mutableStateOf<Skip?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var live by remember { mutableStateOf(false) }
    var view by remember { mutableStateOf<PlayerView?>(null) }
    val skipFocus = remember { FocusRequester() }
    val onNext by rememberUpdatedState(next)
    Fullscreen()

    fun store() {
        val dur = player.duration
        if (!live && dur > 0) save(player.currentPosition / 1000.0, dur / 1000.0)
    }

    // „Weiter“-Knopf der Leiste = nächstes Video der App (ExoPlayer kennt nur das eine).
    val controls = remember {
        object : ForwardingPlayer(player) {
            override fun isCommandAvailable(command: Int) =
                (command == COMMAND_SEEK_TO_NEXT && onNext != null) || super.isCommandAvailable(command)
            override fun getAvailableCommands(): Player.Commands =
                if (onNext == null) super.getAvailableCommands()
                else super.getAvailableCommands().buildUpon().add(COMMAND_SEEK_TO_NEXT).build()
            override fun seekToNext() { store(); onNext?.invoke() }
        }
    }

    DisposableEffect(player) {
        val l = object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_ENDED) { store(); onNext?.invoke() }
            }
            override fun onPlayerError(e: PlaybackException) { error = e.message }
        }
        player.addListener(l)
        onDispose { store(); player.release() }
    }

    LaunchedEffect(Unit) {
        try {
            val v = load()
            live = v.live
            val http = DefaultHttpDataSource.Factory().setDefaultRequestProperties(v.headers)
            v.userAgent?.let { http.setUserAgent(it) }
            val source = DefaultMediaSourceFactory(DefaultDataSource.Factory(ctx, http))
            val subs = v.subtitles.mapIndexed { i, s ->
                val uri = if (s.uri.startsWith("/")) Uri.fromFile(File(s.uri)) else Uri.parse(s.uri)
                MediaItem.SubtitleConfiguration.Builder(uri).setMimeType(s.mime).setLabel(s.label)
                    .setSelectionFlags(if (i == 0) C.SELECTION_FLAG_DEFAULT else 0).build()
            }
            val item = MediaItem.Builder().setUri(v.url).setMimeType(v.mime).setSubtitleConfigurations(subs).build()
            val video = source.createMediaSource(item)
            player.setMediaSource(
                if (v.audio == null) video
                else MergingMediaSource(video, source.createMediaSource(MediaItem.fromUri(v.audio))),
            )
            v.start?.let { player.seekTo((it * 1000).toLong()) }
            player.setPlaybackSpeed(v.speed)
            player.prepare()
            player.playWhenReady = true
            segs = skips()
        } catch (e: Exception) { error = message(e) }
    }

    // Wie bei Netflix: erscheint „Skip …“, bekommt es den Fokus (OK = überspringen); danach wieder das Video.
    val hasSkip = skip != null
    LaunchedEffect(hasSkip) {
        if (hasSkip) runCatching { skipFocus.requestFocus() } else view?.requestFocus()
    }

    // Skip-Knopf und Fortschritt (alle 5 s).
    LaunchedEffect(player) {
        var tick = 0
        while (true) {
            delay(500)
            val pos = player.currentPosition / 1000.0
            skip = segs.firstOrNull { pos >= it.start && pos < it.end - 0.5 }
            if (++tick % 10 == 0 && player.isPlaying) store()
        }
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        AndroidView(
            // Fokus aufs Video: Tasten der Fernbedienung blenden die Leiste ein, Pfeile wandern durch ihre Knöpfe.
            factory = {
                PlayerView(it).apply {
                    this.player = controls
                    keepScreenOn = true
                    // Fernbedienung links/rechts auf der Timeline: 10 s statt 1/20 der Länge.
                    findViewById<DefaultTimeBar>(androidx.media3.ui.R.id.exo_progress)?.setKeyTimeIncrement(10_000)
                    setShowSubtitleButton(true)
                    isFocusable = true
                    post { requestFocus() }
                    view = this
                }
            },
            modifier = Modifier.fillMaxSize(),
        )
        skip?.let { s ->
            // Bubble wie am Desktop („Skip intro“ rechts über der Timeline).
            Text(
                "Skip ${s.label}",
                color = Color.White,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.align(Alignment.BottomEnd).padding(end = 24.dp, bottom = 96.dp)
                    .focusRequester(skipFocus).tvFocus(CircleShape).glass().clip(CircleShape)
                    .clickable { player.seekTo((s.end * 1000).toLong()) }.padding(horizontal = 18.dp, vertical = 10.dp),
            )
        }
        error?.let { e -> Text(e, color = Red, modifier = Modifier.align(Alignment.Center).padding(24.dp)) }
    }
}
