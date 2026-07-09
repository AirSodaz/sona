package com.sona.uniffi.consumer

import com.sona.uniffi.sample.SonaUniffiSmoke
import uniffi.sona_uniffi_bind.defaultConfigJson

object SonaUniffiConsumerSmoke {
    fun defaultConfig(): String = defaultConfigJson()

    fun publishedSmokeTypeName(): String = SonaUniffiSmoke::class.java.name
}
