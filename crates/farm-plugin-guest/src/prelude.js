// The plugin API beyond the guest's `api.on` and `api.host` (docs/PLUGINS.md). The bootstrap
// evaluates this function expression once, when the sandbox is prepared (before any plugin
// code, so the helpers it captures cannot be tampered with), and hands every plugin
// `extend(baseApi, packId, isScript)`. Everything here is a convenience on top of `api.on`: the guest
// only lets a plugin register the hooks it was granted, and the host checks every answer.
(function extend(base, pack, script) {
  var freeze = Object.freeze, isArray = Array.isArray, create = Object.create;
  var ErrorType = Error, TypeErrorType = TypeError, StringType = String;
  var list = function (x) { return isArray(x) ? x : []; };
  var message = function (e) { return e instanceof ErrorType ? e.message : StringType(e); };
  var full = function (name) {
    if (typeof name !== 'string' || name === '') throw new TypeErrorType('a name must be a non-empty string');
    return pack === '' || name.indexOf(':') >= 0 ? name : pack + ':' + name;
  };
  var read = function (request) { return base.host(request); };
  // The read API (#237): one host call per query.
  var state = freeze({
    player: function () { return read({ read: 'player' }); },
    inventory: function () { return read({ read: 'inventory' }); },
    itemCount: function (itemId) { return read({ read: 'itemCount', itemId: itemId }); },
    date: function () { return read({ read: 'date' }); },
    tile: function (x, y, sceneId) { return read({ read: 'tile', x: x, y: y, sceneId: sceneId }); },
    animals: function () { return read({ read: 'animals' }); },
    buildings: function () { return read({ read: 'buildings' }); },
    entities: function (filter) { var f = filter || {}; return read({ read: 'entities', typeId: f.typeId, sceneId: f.sceneId }); },
    timers: function () { return read({ read: 'timers' }); },
    npc: function (npcId) { return read({ read: 'npc', npcId: npcId }); },
    npcs: function () { return read({ read: 'npcs' }); },
    quest: function (questId) { return read({ read: 'quest', questId: questId }); },
    quests: function () { return read({ read: 'quests' }); },
    flag: function (flag) { return read({ read: 'flag', flag: flag }); },
    variable: function (variable) { return read({ read: 'variable', variable: variable }); },
    item: function (itemId) { return read({ read: 'item', itemId: itemId }); },
    crop: function (cropId) { return read({ read: 'crop', cropId: cropId }); },
    definition: function (kind, id) { return read({ read: 'definition', kind: kind, id: id }); },
  });
  var named = function (what, name, fn) {
    return function (a, b, c) {
      try { return fn(a, b, c); } catch (e) { throw new ErrorType(what + ' ' + name + ': ' + message(e)); }
    };
  };
  // A project script (no pack) reports the stack of what it throws (#241).
  var on = !script ? base.on : function (hook, fn) {
    base.on(hook, typeof fn !== 'function' ? fn : function (p) {
      try { return fn(p); } catch (e) {
        throw new ErrorType(e instanceof ErrorType ? e.message + (e.stack ? '\n' + e.stack : '') : StringType(e));
      }
    });
  };
  var timers = create(null), timerRegistered = false;
  return freeze({
    on: on,
    host: base.host,
    state: state,
    // Pack minigames: the handler of `minigame:<name>`, one call per frame for update and draw.
    minigame: function minigame(name, game) {
      if (typeof name !== 'string' || game === null || typeof game !== 'object') {
        throw new TypeErrorType('api.minigame(name, { start, update, draw })');
      }
      var run = function (op, a, b, c) {
        var fn = game[op];
        if (typeof fn !== 'function') {
          if (op === 'start') return null;
          throw new TypeErrorType(op + ': minigame ' + name + ' has no ' + op + ' function');
        }
        try { return fn.call(game, a, b, c); } catch (e) { throw new ErrorType(op + ': ' + message(e)); }
      };
      on('minigame:' + name, function (call) {
        if (call.op === 'start') {
          var first = run('start', call.config, call.info);
          return [first, run('draw', first)];
        }
        var answer = run('update', call.state, call.input, call.dt);
        var next = answer !== null && typeof answer === 'object' && answer.state !== undefined ? answer.state : call.state;
        return [answer, run('draw', next)];
      });
    },
    // Plugin conditions and outcomes (#239).
    condition: function condition(name, fn) {
      if (typeof fn !== 'function') throw new TypeErrorType('api.condition(name, (args, state) => boolean)');
      var f = named('condition', name, fn);
      on('condition:' + full(name), function (call) { return [!!f(call.args, state, call.context)]; });
    },
    outcome: function outcome(name, fn) {
      if (typeof fn !== 'function') throw new TypeErrorType('api.outcome(name, (args, state) => mutations)');
      var f = named('outcome', name, fn);
      on('outcome:' + full(name), function (call) { return list(f(call.args, state)); });
    },
    // Entities, item use and timers (#238).
    entity: function entity(name, handlers) {
      if (handlers === null || typeof handlers !== 'object') {
        throw new TypeErrorType('api.entity(name, { onTick, onInteract, onDayStart })');
      }
      on('entity:' + full(name), function (call) {
        var key = call.op === 'interact' ? 'onInteract' : call.op === 'tick' ? 'onTick' : 'onDayStart';
        var fn = handlers[key];
        if (typeof fn !== 'function') return [];
        var f = named(key, name, function (a, b, c) { return fn.call(handlers, a, b, c); });
        if (call.op === 'interact') return list(f(call.entity, state, call));
        var out = [];
        for (var i = 0; i < call.entities.length; i++) {
          var r = list(f(call.entities[i], state, call));
          for (var j = 0; j < r.length; j++) out.push(r[j]);
        }
        return out;
      });
    },
    itemUse: function itemUse(name, fn) {
      if (typeof fn !== 'function') throw new TypeErrorType('api.itemUse(name, (use, state) => mutations)');
      var f = named('itemUse', name, fn);
      on('use:' + full(name), function (use) { return list(f(use, state)); });
    },
    timer: function timer(name, fn) {
      if (typeof name !== 'string' || typeof fn !== 'function') {
        throw new TypeErrorType('api.timer(name, (timer, state) => mutations)');
      }
      timers[name] = named('timer', name, fn);
      if (!timerRegistered) {
        timerRegistered = true;
        on('timer', function (t) { var f = timers[t.timer] || timers['*']; return f ? list(f(t, state)) : []; });
      }
    },
    // Screens (#240): describe, draw and action.
    screen: function screen(name, def) {
      if (def === null || typeof def !== 'object' || typeof def.draw !== 'function') {
        throw new TypeErrorType('api.screen(name, { draw, action })');
      }
      on('screen:' + full(name), function (call) {
        if (call.op === 'describe') {
          return [{
            title: typeof def.title === 'string' ? def.title : name,
            hotkey: typeof def.hotkey === 'string' ? def.hotkey : null,
          }];
        }
        if (call.op === 'draw') return [named('draw', name, function (a) { return def.draw.call(def, a, call); })(state)];
        if (typeof def.action !== 'function') return [];
        return list(named('action', name, function (a, b) { return def.action.call(def, a, b); })(call.action, state));
      });
    },
  });
})
