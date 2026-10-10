package com.sona.android.application.library

data class ProjectRecord(
    val id: String,
    val name: String,
    val description: String,
    val icon: String,
    val color: String,
    val sortOrder: Long,
    val createdAtEpochMillis: Long,
    val updatedAtEpochMillis: Long,
)

data class CreateProjectRequest(
    val name: String,
    val description: String? = null,
    val icon: String? = null,
    val color: String? = null,
)

interface ProjectWorkspacePort {
    suspend fun listProjects(): List<ProjectRecord>
    suspend fun createProject(request: CreateProjectRequest): ProjectRecord
    suspend fun renameProject(projectId: String, name: String): ProjectRecord?
    suspend fun deleteProject(projectId: String)
}
