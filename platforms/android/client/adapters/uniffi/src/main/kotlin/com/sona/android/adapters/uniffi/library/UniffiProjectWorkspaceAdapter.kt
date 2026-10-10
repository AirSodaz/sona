package com.sona.android.adapters.uniffi.library

import com.sona.android.application.library.CreateProjectRequest
import com.sona.android.application.library.ProjectRecord
import com.sona.android.application.library.ProjectWorkspacePort
import uniffi.sona_uniffi_bind.FfiProjectCreateInputV1
import uniffi.sona_uniffi_bind.FfiProjectRecordV1
import uniffi.sona_uniffi_bind.FfiProjectUpdateInputV1
import uniffi.sona_uniffi_bind.createProjectV1
import uniffi.sona_uniffi_bind.deleteProjectV1
import uniffi.sona_uniffi_bind.loadProjectRepositoryV1
import uniffi.sona_uniffi_bind.updateProjectV1

class UniffiProjectWorkspaceAdapter(
    private val appDataDir: String,
    private val onLocalChange: () -> Unit = {},
) : ProjectWorkspacePort {
    init { require(appDataDir.isNotBlank()) { "Project app data directory must not be blank." } }

    override suspend fun listProjects(): List<ProjectRecord> =
        loadProjectRepositoryV1(appDataDir).projects.map(FfiProjectRecordV1::toApplication)

    override suspend fun createProject(request: CreateProjectRequest): ProjectRecord {
        require(request.name.isNotBlank()) { "Project name must not be blank." }
        return createProjectV1(
            appDataDir,
            FfiProjectCreateInputV1(request.name.trim(), request.description, request.icon, request.color),
        ).toApplication().also { onLocalChange() }
    }

    override suspend fun renameProject(projectId: String, name: String): ProjectRecord? {
        require(projectId.isNotBlank() && name.isNotBlank()) { "Project ID and name must not be blank." }
        return updateProjectV1(
            appDataDir,
            projectId,
            FfiProjectUpdateInputV1(name.trim(), null, null, null),
        )?.toApplication()?.also { onLocalChange() }
    }

    override suspend fun deleteProject(projectId: String) {
        require(projectId.isNotBlank()) { "Project ID must not be blank." }
        deleteProjectV1(appDataDir, projectId)
        onLocalChange()
    }
}

internal fun FfiProjectRecordV1.toApplication() = ProjectRecord(
    id, name, description, icon, color,
    sortOrder.toLongChecked("Project sort order"),
    createdAt.toLongChecked("Project created timestamp"),
    updatedAt.toLongChecked("Project updated timestamp"),
)

private fun ULong.toLongChecked(label: String): Long {
    require(this <= Long.MAX_VALUE.toULong()) { "$label exceeds the Android Long range." }
    return toLong()
}
