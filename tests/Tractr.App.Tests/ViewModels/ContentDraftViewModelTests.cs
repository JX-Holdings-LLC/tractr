using FarmEngine.Schemas;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Tests.ViewModels;

public sealed class ContentDraftViewModelTests
{
    [Theory]
    [InlineData("", "needs a value")]
    [InlineData("NaN", "must be a whole number")]
    [InlineData("1.5", "must be a whole number")]
    [InlineData("-1", "must be from 0 to 100")]
    [InlineData("101", "must be from 0 to 100")]
    public void NumericFieldErrorsNameTheFieldAndKeepTheOriginalValue(string text, string expected)
    {
        var draft = new ContentDraftViewModel(Item.Default, typeof(Item));
        var original = draft.Draft["value"]?.ToJsonString();
        draft.WriteNumber(draft.Draft, "value", "Price", text, integer: true, optional: false, min: 0, max: 100);
        Assert.Contains("Price " + expected, draft.Errors.Single(), StringComparison.Ordinal);
        Assert.Equal(original, draft.Draft["value"]?.ToJsonString());
    }

    [Fact]
    public void JsonDraftTracksEditsRevertsAndCommitsWithoutControls()
    {
        var draft = new ContentDraftViewModel(Item.Default, typeof(Item));
        Assert.False(draft.IsDirty);
        draft.Draft["name"] = "Turnip";
        Assert.True(draft.IsDirty);
        Assert.Equal("Turnip", Assert.IsType<Item>(draft.Commit()).Name);
        draft.Draft["name"] = Item.Default.Name;
        Assert.False(draft.IsDirty);
    }

    [Fact]
    public void InvalidFieldsRemainDirtyAndBlockCommit()
    {
        var draft = new ContentDraftViewModel(Item.Default, typeof(Item));
        draft.Writers.Add(() => draft.Errors.Add("Price must be a number."));
        Assert.True(draft.IsDirty);
        Assert.Null(draft.Preview());
        Assert.Contains("Price must be a number", Assert.Throws<FormatException>(() => draft.Commit()).Message, StringComparison.Ordinal);
        draft.ClearBindings();
        Assert.False(draft.IsDirty);
    }

    [Fact]
    public void InvalidRawJsonReportsItsFieldAndCanBeCorrected()
    {
        var draft = new ContentDraftViewModel(Item.Default, typeof(Item));
        var text = "{";
        draft.JsonFields.Add(new("Name", () => text, "", false, node => draft.Draft["name"] = node));
        Assert.True(draft.IsDirty);
        Assert.Contains("Name is not valid JSON", Assert.Throws<FormatException>(() => draft.Commit()).Message, StringComparison.Ordinal);
        text = "\"Turnip\"";
        Assert.Equal("Turnip", Assert.IsType<Item>(draft.Commit()).Name);
        text = "null";
        Assert.Contains("Name needs a JSON value", Assert.Throws<FormatException>(() => draft.Commit()).Message, StringComparison.Ordinal);
    }
}
