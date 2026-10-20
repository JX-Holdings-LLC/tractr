using FarmEngine.Authoring;
using FarmingRpgMaker.App.Mvvm;
using FarmingRpgMaker.App.Projects;

namespace FarmingRpgMaker.App.ViewModels.EditMode;

/// <summary>
/// The Problems tab: the F# schema and content checks of the open project, collected again when
/// the tab is opened after the project changed, and "Go to" for the problems that have a target.
/// </summary>
public sealed class ProblemsViewModel : ObservableObject
{
    private readonly ProjectWorkspace _workspace;
    private readonly Action<Problem> _navigate;
    private IReadOnlyList<Problem> _problems = [];
    private string _summary = "";
    private bool _hasProject;
    private bool _stale = true;

    /// <param name="workspace">The open project.</param>
    /// <param name="navigate">Opens what a problem is about (the editor's tabs).</param>
    public ProblemsViewModel(ProjectWorkspace workspace, Action<Problem> navigate)
    {
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));
        _navigate = navigate ?? throw new ArgumentNullException(nameof(navigate));
        _workspace.ProjectChanged += OnProjectChanged;
    }

    /// <summary>
    /// The problems the last <see cref="Refresh"/> found, in the F# order. Raised on every
    /// refresh that ran the checks, so the rows are rebuilt even when the list reads the same.
    /// </summary>
    public IReadOnlyList<Problem> Problems => _problems;

    /// <summary>"2 errors · 1 warnings · 3 tips", or "No project open".</summary>
    public string Summary
    {
        get => _summary;
        private set => SetProperty(ref _summary, value);
    }

    /// <summary>False when no project is open (the tab shows no rows at all).</summary>
    public bool HasProject
    {
        get => _hasProject;
        private set => SetProperty(ref _hasProject, value);
    }

    /// <summary>True when the project passed every check (the tab says so).</summary>
    public bool IsClean => HasProject && _problems.Count == 0;

    /// <summary>A row's text: "ERROR · The message".</summary>
    public static string RowTitle(Problem problem)
    {
        ArgumentNullException.ThrowIfNull(problem);
        return $"{problem.SeverityName.ToUpperInvariant()} · {problem.Message}";
    }

    /// <summary>The name of a row's "Go to" button: every row has one, so the name says which problem it opens.</summary>
    public static string GoToName(Problem problem)
    {
        ArgumentNullException.ThrowIfNull(problem);
        return $"Go to: {problem.Message}";
    }

    /// <summary>Stops following the project (the editor that built this tab was replaced).</summary>
    public void Retire() => _workspace.ProjectChanged -= OnProjectChanged;

    /// <summary>Runs the checks again when the project changed since the last time.</summary>
    public void Refresh()
    {
        if (!_stale) return;
        _stale = false;
        if (_workspace.Current is not { } project)
        {
            HasProject = false;
            Summary = "No project open";
            Show([]);
            return;
        }

        var problems = FarmEngine.Authoring.Problems.Collect(project);
        var errors = problems.Count(problem => problem.IsError);
        var warnings = problems.Count(problem => problem.IsWarning);
        HasProject = true;
        Summary = $"{errors} errors · {warnings} warnings · {problems.Count - errors - warnings} tips";
        Show(problems);
    }

    /// <summary>Opens what <paramref name="problem"/> is about; problems without a target go nowhere.</summary>
    public void GoTo(Problem problem)
    {
        ArgumentNullException.ThrowIfNull(problem);
        if (problem.HasTarget) _navigate(problem);
    }

    private void Show(IReadOnlyList<Problem> problems)
    {
        _problems = problems;
        OnPropertiesChanged(nameof(Problems), nameof(IsClean));
    }

    private void OnProjectChanged(object? sender, ProjectChangedEventArgs e) => _stale = true;
}
