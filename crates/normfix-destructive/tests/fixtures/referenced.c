static int	helper(void)
{
	return (42);
}

int	public_api(void)
{
	return (helper());
}

int	never_a_candidate(void)
{
	return (0);
}
