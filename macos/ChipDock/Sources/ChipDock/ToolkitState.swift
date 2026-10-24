import Observation

@MainActor
@Observable
final class ToolkitState {
    var selectedTool = WorkbenchTool.flash
}
