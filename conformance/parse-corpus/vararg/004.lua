    local function foo (...extra)
      return function (...) extra = nil end
    end
  