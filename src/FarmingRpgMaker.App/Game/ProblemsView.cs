using System.ComponentModel;
using Avalonia;
using Avalonia.Controls;
using FarmEngine.Authoring;
using FarmingRpgMaker.App.Projects;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Game;

/// <summary>The Problems tab: shows the rows of its <see cref="ProblemsViewModel"/>.</summary>
public sealed class ProblemsView : UserControl, IRetirable
{
    private readonly StackPanel _rows = new() { Name = "ProblemRows", Spacing = 8 };
    private readonly TextBlock _summary = Ui.Text("", "h2");

    public ProblemsView(ProjectWorkspace workspace, Action<Problem> navigate)
    {
        ViewModel = new ProblemsViewModel(workspace, navigate);
        Name = "ProblemsView";
        _summary.Name = "ProblemsSummary";
        var stack = new StackPanel { Spacing = 12, Margin = new Thickness(20) };
        stack.Children.Add(_summary);
        stack.Children.Add(Ui.Wrapped("Errors block export. Warnings and tips help you improve the project.", "muted", "small"));
        stack.Children.Add(_rows);
        Content = new ScrollViewer { Content = stack };
        ViewModel.PropertyChanged += OnViewModelChanged;
    }

    public ProblemsViewModel ViewModel { get; }

    /// <summary>Stops following the project (the editor that built this view was replaced).</summary>
    public void Retire() => ViewModel.Retire();

    public void Refresh() => ViewModel.Refresh();

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(ProblemsViewModel.Summary)) _summary.Text = ViewModel.Summary;
        if (e.PropertyName == nameof(ProblemsViewModel.Problems)) Render();
    }

    private void Render()
    {
        _rows.Children.Clear();
        if (!ViewModel.HasProject) return;
        if (ViewModel.IsClean)
        {
            _rows.Children.Add(Ui.Empty("IconStar", "No problems found", "Your project passed the current checks."));
            return;
        }

        var problems = ViewModel.Problems;
        for (var index = 0; index < problems.Count; index++)
        {
            var problem = problems[index];
            var title = Ui.Wrapped(ProblemsViewModel.RowTitle(problem), "small");
            if (problem.IsError) title.Classes.Add("error");
            var path = Ui.Text(problem.Path, "muted", "small");
            var text = Ui.VStack(4, title, path);
            if (problem.HasTarget)
            {
                var go = Ui.Button("Go to", () => ViewModel.GoTo(problem), "tool", "small");
                go.Name = $"ProblemGo_{index}";
                Avalonia.Automation.AutomationProperties.SetName(go, ProblemsViewModel.GoToName(problem));
                _rows.Children.Add(Ui.Row(text, go));
            }
            else
            {
                _rows.Children.Add(Ui.Row(text));
            }
        }
    }
}
