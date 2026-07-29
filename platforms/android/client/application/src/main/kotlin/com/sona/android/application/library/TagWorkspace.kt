package com.sona.android.application.library

data class TagRecord(
    val id: String,
    val name: String,
    val description: String,
    val icon: String,
    val color: String,
    val sortOrder: Long,
    val createdAtEpochMillis: Long,
    val updatedAtEpochMillis: Long,
)

data class CreateTagRequest(
    val name: String,
    val description: String? = null,
    val icon: String? = null,
    val color: String? = null,
)

interface TagWorkspacePort {
    suspend fun listTags(): List<TagRecord>
    suspend fun createTag(request: CreateTagRequest): TagRecord
    suspend fun renameTag(tagId: String, name: String): TagRecord?
    suspend fun deleteTag(tagId: String)
}
