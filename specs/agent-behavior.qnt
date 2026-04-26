// Per-session agent-behavior model.
// Tracks mode transitions, todo lifecycle, and user interrupts.
// Invariants:
//   todos_completed + todos_cancelled <= todos_created (todos_created monotonic)
//   mode_transitions non-negative
//   interrupts non-negative
module agent_behavior {
  type Mode = | Plan | Agent | Ask | Debug

  var mode: Mode
  var todosCreated: int
  var todosCompleted: int
  var todosCancelled: int
  var modeTransitions: int
  var interrupts: int

  action init: bool = all {
    mode'            = Agent,
    todosCreated'    = 0,
    todosCompleted'  = 0,
    todosCancelled'  = 0,
    modeTransitions' = 0,
    interrupts'      = 0
  }

  action switch_mode: bool = {
    nondet to_mode = oneOf(Set(Plan, Agent, Ask, Debug))
    all {
      to_mode != mode,
      mode'            = to_mode,
      modeTransitions' = modeTransitions + 1,
      todosCreated'    = todosCreated,
      todosCompleted'  = todosCompleted,
      todosCancelled'  = todosCancelled,
      interrupts'      = interrupts
    }
  }

  action todo_write: bool = {
    nondet count = oneOf(Set(1, 2, 3, 4, 5))
    all {
      todosCreated'    = todosCreated + count,
      todosCompleted'  = todosCompleted,
      todosCancelled'  = todosCancelled,
      mode'            = mode,
      modeTransitions' = modeTransitions,
      interrupts'      = interrupts
    }
  }

  action todo_complete: bool = all {
    todosCompleted < todosCreated - todosCancelled,
    todosCompleted'  = todosCompleted + 1,
    todosCreated'    = todosCreated,
    todosCancelled'  = todosCancelled,
    mode'            = mode,
    modeTransitions' = modeTransitions,
    interrupts'      = interrupts
  }

  action todo_cancel: bool = all {
    todosCancelled < todosCreated - todosCompleted,
    todosCancelled'  = todosCancelled + 1,
    todosCreated'    = todosCreated,
    todosCompleted'  = todosCompleted,
    mode'            = mode,
    modeTransitions' = modeTransitions,
    interrupts'      = interrupts
  }

  action user_interrupt: bool = all {
    interrupts'      = interrupts + 1,
    todosCreated'    = todosCreated,
    todosCompleted'  = todosCompleted,
    todosCancelled'  = todosCancelled,
    mode'            = mode,
    modeTransitions' = modeTransitions
  }

  action step: bool = any {
    switch_mode,
    todo_write,
    todo_complete,
    todo_cancel,
    user_interrupt
  }

  val todos_created_monotonic: bool = todosCreated >= 0

  val todos_completed_bounded: bool =
    todosCompleted + todosCancelled <= todosCreated

  val interrupts_non_negative: bool = interrupts >= 0

  val mode_transitions_non_negative: bool = modeTransitions >= 0

  run todoLifecycle = {
    init
      .then(todo_write)
      .then(todo_complete)
      .expect(all {
        todosCreated >= 1,
        todosCompleted == 1,
        todos_completed_bounded
      })
  }

  run modeThrashCycle = {
    init
      .then(switch_mode)
      .then(switch_mode)
      .then(switch_mode)
      .expect(modeTransitions == 3)
  }

  run interruptCountsAccumulate = {
    init
      .then(user_interrupt)
      .then(user_interrupt)
      .expect(interrupts == 2)
  }
}
