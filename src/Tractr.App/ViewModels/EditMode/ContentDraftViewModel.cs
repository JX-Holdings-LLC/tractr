using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using FarmEngine.Authoring.Net;

namespace Tractr.App.ViewModels.EditMode;

/// <summary>
/// A schema record's editable JSON, dirty state and validation. Generated controls register
/// writers to collect text; tests can edit the JSON or register ordinary delegates directly.
/// No control owns the draft or decides whether it can be committed.
/// </summary>
public sealed class ContentDraftViewModel : IContentDraft
{
    public sealed record JsonField(string Label, Func<string?> Read, string Original, bool Nullable, Action<JsonNode?> Replace);

    private readonly Type _entityType;
    private string _initialJson;

    public ContentDraftViewModel(object entity, Type entityType)
    {
        _entityType = entityType;
        Draft = RecordJson.ToNode(entity, entityType) as JsonObject
            ?? throw new InvalidOperationException("Content did not serialize to an object.");
        _initialJson = Draft.ToJsonString();
    }

    public JsonObject Draft { get; }

    public List<Action> Writers { get; } = [];

    public List<JsonField> JsonFields { get; } = [];

    public List<string> Errors { get; } = [];

    public void AcceptInitial() => _initialJson = Draft.ToJsonString();

    public bool IsDirty
    {
        get
        {
            CollectFields();
            return Errors.Count > 0 || JsonFields.Any(entry => (entry.Read() ?? "") != entry.Original)
                || Draft.ToJsonString() != _initialJson;
        }
    }

    private void CollectFields()
    {
        Errors.Clear();
        foreach (var write in Writers) write();
    }

    public List<string> Flush()
    {
        CollectFields();
        // Inner JSON fields are applied first, so an explicitly edited parent wins.
        for (var i = JsonFields.Count - 1; i >= 0; i--)
        {
            var field = JsonFields[i];
            var text = field.Read() ?? "";
            if (text == field.Original) continue;
            try
            {
                var node = text.Trim().Length == 0 ? null : JsonNode.Parse(text,
                    documentOptions: new JsonDocumentOptions { AllowTrailingCommas = true, CommentHandling = JsonCommentHandling.Skip });
                if (node is null && !field.Nullable) Errors.Add($"{field.Label} needs a JSON value.");
                else field.Replace(node);
            }
            catch (JsonException error)
            {
                Errors.Add($"{field.Label} is not valid JSON: {error.Message}");
            }
        }

        return [.. Errors];
    }

    public object Commit()
    {
        var errors = Flush();
        if (errors.Count > 0) throw new FormatException(string.Join(" ", errors));
        try
        {
            return RecordJson.FromNode(Draft, _entityType);
        }
        catch (FormatException error)
        {
            throw new JsonException(error.Message, error);
        }
    }

    public object? Preview()
    {
        CollectFields();
        if (Errors.Count > 0) return null;
        try
        {
            return RecordJson.FromNode(Draft, _entityType);
        }
        catch (FormatException)
        {
            return null;
        }
    }

    /// <summary>Preserves absent fields and supports typing back to the original value after a preview.</summary>
    public void RegisterWriter(JsonObject target, string key, Func<bool> unchanged, Action write)
    {
        var had = target.ContainsKey(key);
        var before = target[key]?.DeepClone();
        Writers.Add(() =>
        {
            if (!unchanged()) write();
            else if (!had) target.Remove(key);
            else if (!JsonNode.DeepEquals(target[key], before)) target[key] = before?.DeepClone();
        });
    }

    public void WriteNumber(JsonObject target, string key, string label, string? text, bool integer, bool optional, double? min, double? max)
    {
        var raw = (text ?? "").Trim();
        if (raw.Length == 0)
        {
            if (optional) target[key] = null;
            else Errors.Add($"{label} needs a value.");
            return;
        }
        if (!double.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out var value) || !double.IsFinite(value)
            || (integer && value != Math.Floor(value)))
        {
            Errors.Add($"{label} must be {(integer ? "a whole number" : "a number")}.");
            return;
        }
        if ((min is { } low && value < low) || (max is { } high && value > high))
        {
            Errors.Add((min, max) switch
            {
                ({ } lower, { } upper) => $"{label} must be from {FarmEngine.Authoring.JsNumber.format(lower)} to {FarmEngine.Authoring.JsNumber.format(upper)}.",
                ({ } lower, null) => $"{label} must be at least {FarmEngine.Authoring.JsNumber.format(lower)}.",
                (null, { } upper) => $"{label} must be at most {FarmEngine.Authoring.JsNumber.format(upper)}.",
                _ => $"{label} is out of range.",
            });
            return;
        }
        target[key] = JsonValue.Create(value);
    }

    public void ClearBindings()
    {
        Writers.Clear();
        JsonFields.Clear();
    }
}
