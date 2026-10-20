using FarmingRpgMaker.App.Mvvm;

namespace FarmingRpgMaker.App.ViewModels.EditMode;

/// <summary>
/// The question a form asks before something would replace fields the creator has not saved
/// (#87): another entry or panel chosen, or a jump from Problems or search. "Save and
/// continue" saves the fields first, "Discard changes" drops them, "Keep editing" stays. The
/// owner calls <see cref="Ask"/> with what would happen next and does nothing until the creator
/// answers. <c>Game/DraftBar</c> shows it.
/// </summary>
public sealed class DraftPrompt : ObservableObject
{
    private readonly Func<bool> _save;
    private readonly Action _discard;
    private Action? _next;
    private string _text = "";
    private bool _isAsking;

    /// <param name="save">Saves the fields; false when they could not be saved (the form says why).</param>
    /// <param name="discard">Drops the fields (reloads them from the project).</param>
    public DraftPrompt(Func<bool> save, Action discard)
    {
        _save = save ?? throw new ArgumentNullException(nameof(save));
        _discard = discard ?? throw new ArgumentNullException(nameof(discard));
    }

    /// <summary>"You have unsaved changes to … Save them before you go on?"</summary>
    public string Text
    {
        get => _text;
        private set => SetProperty(ref _text, value);
    }

    /// <summary>True while the question waits for an answer.</summary>
    public bool IsAsking
    {
        get => _isAsking;
        private set => SetProperty(ref _isAsking, value);
    }

    /// <summary>Asks about the unsaved changes to <paramref name="what"/>; <paramref name="next"/> runs once they are saved or dropped.</summary>
    public void Ask(string what, Action next)
    {
        _next = next;
        Text = $"You have unsaved changes to {what}. Save them before you go on?";
        IsAsking = true;
    }

    /// <summary>Stops asking ("Keep editing", or the fields were saved or reverted another way).</summary>
    public void Hide()
    {
        _next = null;
        IsAsking = false;
    }

    // Saving or discarding reloads the form, which hides the question: take what comes next first.

    /// <summary>"Save and continue": saves, then goes on; when saving fails the form says why and the question stays.</summary>
    public void SaveAndContinue()
    {
        var next = _next;
        if (!_save())
        {
            _next = next;
            IsAsking = true;
            return;
        }

        Hide();
        next?.Invoke();
    }

    /// <summary>"Discard changes": drops the fields, then goes on.</summary>
    public void DiscardAndContinue()
    {
        var next = _next;
        _discard();
        Hide();
        next?.Invoke();
    }
}
