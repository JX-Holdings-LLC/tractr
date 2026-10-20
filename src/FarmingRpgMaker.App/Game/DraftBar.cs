using System.ComponentModel;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Game;

/// <summary>
/// Shows a form's <see cref="DraftPrompt"/>: the question it asks before something would
/// replace fields the creator has not saved (#87), with "Save and continue", "Discard changes"
/// and "Keep editing".
/// </summary>
internal sealed class DraftBar : Border
{
    private readonly TextBlock _text = Ui.Wrapped("", "small");

    /// <param name="prefix">Names the bar and its buttons (<c>{prefix}DraftBar</c>, <c>{prefix}DraftSave</c>…).</param>
    /// <param name="save">Saves the fields; false when they could not be saved (the form says why).</param>
    /// <param name="discard">Drops the fields (rebuilds them from the project).</param>
    public DraftBar(string prefix, Func<bool> save, Action discard)
        : this(prefix, new DraftPrompt(save, discard))
    {
    }

    /// <param name="prefix">Names the bar and its buttons (<c>{prefix}DraftBar</c>, <c>{prefix}DraftSave</c>…).</param>
    /// <param name="prompt">The form's question (a view model owns it).</param>
    public DraftBar(string prefix, DraftPrompt prompt)
    {
        Prompt = prompt ?? throw new ArgumentNullException(nameof(prompt));
        Name = $"{prefix}DraftBar";
        IsVisible = false;
        _text.Name = $"{prefix}DraftText";
        AutomationProperties.SetLiveSetting(_text, AutomationLiveSetting.Assertive);
        var saveButton = Ui.Button("Save and continue", prompt.SaveAndContinue, "accent", "small");
        saveButton.Name = $"{prefix}DraftSave";
        var discardButton = Ui.Button("Discard changes", prompt.DiscardAndContinue, "tool", "small");
        discardButton.Name = $"{prefix}DraftDiscard";
        var keep = Ui.Button("Keep editing", prompt.Hide, "tool", "small");
        keep.Name = $"{prefix}DraftKeep";
        Child = Ui.VStack(8, _text, Ui.HStack(8, saveButton, discardButton, keep));
        Classes.Add("notice");
        Margin = new Thickness(0, 0, 0, 4);
        prompt.PropertyChanged += OnPromptChanged;
    }

    public DraftPrompt Prompt { get; }

    /// <summary>Asks about the unsaved changes to <paramref name="what"/>; <paramref name="next"/> runs once they are saved or dropped.</summary>
    public void Ask(string what, Action next) => Prompt.Ask(what, next);

    /// <summary>Stops asking (the fields were saved or reverted another way).</summary>
    public void Hide() => Prompt.Hide();

    private void OnPromptChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(DraftPrompt.Text)) _text.Text = Prompt.Text;
        if (e.PropertyName == nameof(DraftPrompt.IsAsking)) IsVisible = Prompt.IsAsking;
    }
}
