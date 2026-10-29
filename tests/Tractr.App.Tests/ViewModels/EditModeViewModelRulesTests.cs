using System.Reflection;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Tests.ViewModels;

/// <summary>The rule of docs/dev/EDITOR-ARCHITECTURE.md: the Edit Mode view models have no Avalonia types.</summary>
public sealed class EditModeViewModelRulesTests
{
    private const BindingFlags Everything = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;

    [Fact]
    public void EditModeViewModelsUseNoAvaloniaTypes()
    {
        var types = typeof(ProblemsViewModel).Assembly.GetTypes()
            .Where(type => type.Namespace == typeof(ProblemsViewModel).Namespace)
            .ToList();
        Assert.NotEmpty(types);
        var offenders = new List<string>();
        foreach (var type in types)
        {
            foreach (var used in UsedTypes(type))
            {
                if (IsAvalonia(used)) offenders.Add($"{type.Name}: {used.FullName}");
            }
        }

        Assert.Empty(offenders);
    }

    private static IEnumerable<Type> UsedTypes(Type type)
    {
        if (type.BaseType is { } baseType) yield return baseType;
        foreach (var face in type.GetInterfaces()) yield return face;
        foreach (var field in type.GetFields(Everything)) yield return field.FieldType;
        foreach (var property in type.GetProperties(Everything)) yield return property.PropertyType;
        foreach (var method in type.GetMethods(Everything).Cast<MethodBase>().Concat(type.GetConstructors(Everything)))
        {
            if (method is MethodInfo info) yield return info.ReturnType;
            foreach (var parameter in method.GetParameters()) yield return parameter.ParameterType;
        }
    }

    private static bool IsAvalonia(Type type)
    {
        if (type.Assembly.GetName().Name?.StartsWith("Avalonia", StringComparison.Ordinal) == true) return true;
        if (type.HasElementType && type.GetElementType() is { } element && IsAvalonia(element)) return true;
        return type.IsGenericType && type.GetGenericArguments().Any(IsAvalonia);
    }
}
