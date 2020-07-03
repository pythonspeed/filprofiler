subroutine fortranallocate()
  REAL, DIMENSION(:), ALLOCATABLE :: A
  ALLOCATE ( A(10000000) )
  DEALLOCATE ( A )
end subroutine fortranallocate
