package dev.hypernavigator.examples.counter

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.Button
import androidx.compose.material.ButtonDefaults
import androidx.compose.material.MaterialTheme
import androidx.compose.material.Surface
import androidx.compose.material.Text
import androidx.compose.material.darkColors
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Edit this label, rebuild the example, and press Reload in HyperNavigator (HN-006). */
const val BUILD_LABEL = "Build 1"

private val Muted = Color(0xFFA1A1AA)
private val CardBackground = Color(0xFF27272A)
private val CardBorder = Color(0xFF3F3F46)

@Composable
fun CounterApp(folderListing: FolderListing? = null) {
    MaterialTheme(colors = darkColors(primary = Color(0xFFF4F4F5), onPrimary = Color(0xFF18181B))) {
        Surface(modifier = Modifier.fillMaxSize(), color = Color(0xFF111113), contentColor = Color(0xFFF4F4F5)) {
            Column(
                modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(32.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text("REMOTE KOTLIN COMPOSE APPLICATION", color = Muted, fontSize = 13.sp, letterSpacing = 1.sp)
                Spacer(Modifier.height(12.dp))
                Text("Hello from HyperNavigator!", fontSize = 26.sp, fontWeight = FontWeight.SemiBold)
                Spacer(Modifier.height(8.dp))
                Text(
                    "This interface is a Kotlin @Composable compiled to Wasm and drawn by Compose on a canvas.",
                    color = Muted,
                    textAlign = TextAlign.Center,
                )
                Spacer(Modifier.height(24.dp))
                Counter()
                Spacer(Modifier.height(12.dp))
                Text(BUILD_LABEL, color = Muted, fontSize = 12.sp)
                Spacer(Modifier.height(32.dp))
                FolderListingPanel(folderListing)
            }
        }
    }
}

@Composable
private fun Counter() {
    var count by remember { mutableIntStateOf(0) }
    Column(
        modifier = Modifier
            .widthIn(min = 260.dp)
            .background(CardBackground, RoundedCornerShape(12.dp))
            .border(1.dp, CardBorder, RoundedCornerShape(12.dp))
            .padding(20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("$count", fontSize = 40.sp, fontWeight = FontWeight.Medium)
        Text("Clicked $count times", color = Muted, fontSize = 13.sp)
        Spacer(Modifier.height(12.dp))
        Button(
            onClick = { count++ },
            shape = RoundedCornerShape(50),
            colors = ButtonDefaults.buttonColors(),
        ) {
            Text("+  Increment counter", modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp))
        }
    }
}

@Composable
private fun FolderListingPanel(listing: FolderListing?) {
    Column(
        modifier = Modifier
            .widthIn(max = 560.dp)
            .fillMaxWidth()
            .background(CardBackground, RoundedCornerShape(12.dp))
            .border(1.dp, CardBorder, RoundedCornerShape(12.dp))
            .padding(16.dp),
    ) {
        Text("Shared folder names", fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(6.dp))
        if (listing == null) {
            Text(
                "Nothing shared. Use \"Share folder names\" in the HyperNavigator toolbar to grant this app " +
                    "a one-time, read-only list of names and types.",
                color = Muted,
                fontSize = 13.sp,
            )
            return@Column
        }
        Text(
            "${listing.entries.size} entr${if (listing.entries.size == 1) "y" else "ies"} received from the Host" +
                if (listing.truncated) " (limited to the first ${listing.entries.size})" else "",
            color = Muted,
            fontSize = 13.sp,
        )
        Spacer(Modifier.height(8.dp))
        if (listing.entries.isEmpty()) {
            Text("The folder is empty.", color = Muted, fontSize = 13.sp)
        }
        listing.entries.forEach { entry ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 3.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(entry.name, fontFamily = FontFamily.Monospace, fontSize = 13.sp, modifier = Modifier.weight(1f))
                Text(entry.kind, color = Muted, fontSize = 12.sp)
            }
        }
    }
}
