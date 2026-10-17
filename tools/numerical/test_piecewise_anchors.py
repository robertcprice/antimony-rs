import copy,math,unittest
import piecewise_anchors as a
import anchors as base

class PiecewiseAnchorTests(unittest.TestCase):
    def test_one_exact_manifest(self):self.assertEqual(len(a.load_manifest()['cases']),1)
    def test_initial_mid_final_values(self):
        for t,v in [(0,0),(2,2),(5,12.5),(6,17.5),(10,37.5)]:self.assertEqual(a.expected_values(a.ID,t)['S'],v)
    def test_grid_includes_switch(self):
        self.assertEqual(base.GRID,{'start':0,'end':10,'steps':100});self.assertIn(5,[i/10 for i in range(101)])
    def test_continuous_boundary_and_rate(self):
        self.assertEqual(0.5*5*5,5*5-12.5);self.assertEqual(5,5)
        h=1e-5;left=(a.expected_values(a.ID,5)['S']-a.expected_values(a.ID,5-h)['S'])/h;right=(a.expected_values(a.ID,5+h)['S']-a.expected_values(a.ID,5)['S'])/h
        self.assertAlmostEqual(left,5,places=4);self.assertAlmostEqual(right,5,places=4)
    def test_bad_time_rejected(self):
        for t in [True,-1,11,math.inf,math.nan]:
            with self.assertRaises(ValueError):a.expected_values(a.ID,t)
    def test_unknown_case_rejected(self):
        with self.assertRaises(ValueError):a.expected_values('other',0)
    def test_full_projection_contract(self):a.validate_preconditions(a.ID,a.expected_projection(a.ID))
    def test_switch_or_order_mutation_rejected(self):
        for index in (0,1,2):
            p=a.expected_projection(a.ID);p['reactions'][0]['math']['children'][index]={'type':256,'integer':4}
            with self.assertRaises(ValueError):a.validate_preconditions(a.ID,p)
    def test_omitted_else_rejected(self):
        p=a.expected_projection(a.ID);p['reactions'][0]['math']['children'].pop()
        with self.assertRaises(ValueError):a.validate_preconditions(a.ID,p)
    def test_volume_or_initial_mutation_rejected(self):
        p=a.expected_projection(a.ID);p['compartments'][0]['size']=2
        with self.assertRaises(ValueError):a.validate_preconditions(a.ID,p)
        p=a.expected_projection(a.ID);p['species'][0]['initial_concentration']=1
        with self.assertRaises(ValueError):a.validate_preconditions(a.ID,p)
    def test_solver_and_tolerances_unchanged(self):
        m=a.load_manifest();self.assertEqual(m['solver'],base.SOLVER);self.assertEqual(m['comparison'],base.COMPARISON)
    def test_nonfinite_row_rejected(self):
        row={'time':1,**a.expected_values(a.ID,1)};row['S']=math.nan
        with self.assertRaises(ValueError):a.invariant_values(a.ID,row)

if __name__=='__main__':unittest.main()
