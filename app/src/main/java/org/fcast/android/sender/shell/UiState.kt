package org.fcast.android.sender.shell

import org.fcast.android.sender.runtime.BackendKind

/**
 * Coarse UI state observed by the Android shell. Maps 1:1 onto Slint's
 * AppState enum (see refactor step 08.8).
 *
 * Why sealed: the `when` over all variants is checked exhaustively at
 * compile time — adding a new variant forces every caller to handle it.
 */
sealed class UiState {

    /** No backend running, no capture, no consent in flight. */
    data object Disconnected : UiState()

    /** Backend lifecycle in progress; UI shows a spinner. */
    data class Starting(val kind: BackendKind) : UiState()

    /** Backend reports state="running". UI can offer "Cast" buttons. */
    data class Connected(val kind: BackendKind, val message: String?) : UiState()

    /** Backend or coordinator returned an error. */
    data class Error(val message: String) : UiState()

    /** User started capturing the screen. */
    data class Casting(val kind: BackendKind, val widthPx: Int, val heightPx: Int) : UiState()
}
