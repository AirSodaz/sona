package com.sona.android.adapters.uniffi.library

import com.sona.android.application.library.CreateTagRequest
import com.sona.android.application.library.TagRecord
import com.sona.android.application.library.TagWorkspacePort
import uniffi.sona_uniffi_bind.FfiTagCreateInputV1
import uniffi.sona_uniffi_bind.FfiTagRecordV1
import uniffi.sona_uniffi_bind.FfiTagUpdateInputV1
import uniffi.sona_uniffi_bind.createTagV1
import uniffi.sona_uniffi_bind.deleteTagV1
import uniffi.sona_uniffi_bind.loadTagRepositoryV1
import uniffi.sona_uniffi_bind.updateTagV1

class UniffiTagWorkspaceAdapter(
    private val appDataDir: String,
    private val onLocalChange: () -> Unit = {},
) : TagWorkspacePort {
    init { require(appDataDir.isNotBlank()) { "Tag app data directory must not be blank." } }
    override suspend fun listTags(): List<TagRecord> =
        loadTagRepositoryV1(appDataDir).tags.map(FfiTagRecordV1::toApplication)

    override suspend fun createTag(request: CreateTagRequest): TagRecord {
        require(request.name.isNotBlank()) { "Tag name must not be blank." }
        return createTagV1(
            appDataDir,
            FfiTagCreateInputV1(request.name.trim(), request.description, request.icon, request.color),
        ).toApplication().also { onLocalChange() }
    }

    override suspend fun renameTag(tagId: String, name: String): TagRecord? {
        require(tagId.isNotBlank() && name.isNotBlank()) { "Tag ID and name must not be blank." }
        return updateTagV1(
            appDataDir,
            tagId,
            FfiTagUpdateInputV1(name.trim(), null, null, null),
        )?.toApplication()?.also { onLocalChange() }
    }

    override suspend fun deleteTag(tagId: String) {
        require(tagId.isNotBlank()) { "Tag ID must not be blank." }
        deleteTagV1(appDataDir, tagId)
        onLocalChange()
    }
}

internal fun FfiTagRecordV1.toApplication() = TagRecord(
    id, name, description, icon, color,
    sortOrder.toLongChecked("Tag sort order"),
    createdAt.toLongChecked("Tag created timestamp"),
    updatedAt.toLongChecked("Tag updated timestamp"),
)

private fun ULong.toLongChecked(label: String): Long {
    require(this <= Long.MAX_VALUE.toULong()) { "$label exceeds the Android Long range." }
    return toLong()
}
