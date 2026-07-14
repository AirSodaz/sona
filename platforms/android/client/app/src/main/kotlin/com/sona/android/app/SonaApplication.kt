package com.sona.android.app

import android.app.Application
import com.sona.android.app.composition.SonaAppContainer

class SonaApplication : Application() {
    val container: SonaAppContainer by lazy { SonaAppContainer(this) }
}
