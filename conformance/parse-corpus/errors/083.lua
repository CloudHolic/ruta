  -- create light udata
  local x = D.upvalueid(function () return debug end, 1)
  D.setuservalue(x, {})
