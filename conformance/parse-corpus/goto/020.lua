    global foo <const>;
    function foo (x)    -- ERROR: foo is read-only
      return
    end
  