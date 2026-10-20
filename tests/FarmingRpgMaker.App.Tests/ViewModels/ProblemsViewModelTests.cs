using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Tests.ViewModels;

/// <summary>The Problems tab's rules, driven without the editor.</summary>
public sealed class ProblemsViewModelTests
{
    [Fact]
    public void CollectsOnlyWhenTheProjectChanged()
    {
        using var test = new TestWorkspace();
        var opened = new List<Problem>();
        var viewModel = new ProblemsViewModel(test.Workspace, opened.Add);
        var raised = new List<string?>();
        viewModel.PropertyChanged += (_, e) => raised.Add(e.PropertyName);
        Assert.Empty(viewModel.Problems);
        Assert.Equal("", viewModel.Summary);

        viewModel.Refresh();
        Assert.True(viewModel.HasProject);
        Assert.Contains(nameof(ProblemsViewModel.Problems), raised);
        var clean = viewModel.Problems;

        // Nothing changed: the checks are not run again.
        raised.Clear();
        viewModel.Refresh();
        Assert.Empty(raised);

        var npc = test.Project.Npcs[0];
        test.Workspace.Apply(Edits.UpsertNpc(npc.WithSceneId("missing-scene")));
        viewModel.Refresh();
        Assert.True(viewModel.Problems.Count > clean.Count);
        var errors = viewModel.Problems.Count(problem => problem.IsError);
        var warnings = viewModel.Problems.Count(problem => problem.IsWarning);
        Assert.Equal($"{errors} errors · {warnings} warnings · {viewModel.Problems.Count - errors - warnings} tips", viewModel.Summary);
        Assert.False(viewModel.IsClean);

        var problem = viewModel.Problems.First(p => p.TargetKind == "npc" && p.TargetId == npc.Id);
        Assert.StartsWith("ERROR · ", ProblemsViewModel.RowTitle(problem), StringComparison.Ordinal);
        Assert.Equal($"Go to: {problem.Message}", ProblemsViewModel.GoToName(problem));
        viewModel.GoTo(problem);
        Assert.Same(problem, Assert.Single(opened));

        // A retired tab stops following the project.
        viewModel.Retire();
        test.Workspace.Undo();
        viewModel.Refresh();
        Assert.Contains(viewModel.Problems, p => p.TargetId == npc.Id);
    }

    [Fact]
    public void ProblemsWithoutATargetGoNowhere()
    {
        using var test = new TestWorkspace();
        var opened = new List<Problem>();
        var viewModel = new ProblemsViewModel(test.Workspace, opened.Add);
        viewModel.Refresh();
        foreach (var problem in viewModel.Problems.Where(p => !p.HasTarget)) viewModel.GoTo(problem);
        Assert.Empty(opened);
    }
}
