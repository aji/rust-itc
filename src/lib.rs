//! Interval tree clocks, or ITC, are a causality tracking primitive similar to
//! vector clocks but specifically designed for use in systems with relatively
//! frequent cluster membership changes.

use std::cmp::Ord;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone)]
pub enum Ident {
    Zero,
    One,
    Tuple(Rc<Ident>, Rc<Ident>),
}

impl Ident {
    pub fn split(&self) -> (Ident, Ident) {
        use Ident::*;

        match *self {
            Zero => {
                (
                    Zero,
                    Zero
                )
            },

            One => {
                (
                    Tuple(Rc::new(One), Rc::new(Zero)),
                    Tuple(Rc::new(Zero), Rc::new(One))
                )
            },

            Tuple(ref i1, ref i2) => match (&**i1, &**i2) {
                (&Zero, ref id) => {
                    let (l, r) = id.split();
                    (
                        Tuple(i1.clone(), Rc::new(l)),
                        Tuple(i1.clone(), Rc::new(r))
                    )
                },

                (ref id, &Zero) => {
                    let (l, r) = id.split();
                    (
                        Tuple(Rc::new(l), i2.clone()),
                        Tuple(Rc::new(r), i2.clone())
                    )
                },

                _ => {
                    (
                        Tuple(i1.clone(), Rc::new(Zero)),
                        Tuple(Rc::new(Zero), i2.clone())
                    )
                },
            },
        }
    }

    pub fn norm(self) -> Ident {
        use Ident::*;

        match self {
            Tuple(i1, i2) => match (&*i1, &*i2) {
                (&Zero, &Zero) => Zero,
                (&One, &One) => One,
                _ => Tuple(i1, i2),
            },

            _ => self
        }
    }

    pub fn sum(&self, other: &Ident) -> Ident {
        use Ident::*;

        if let Zero = *self {
            return other.clone();
        }

        if let Zero = *other {
            return self.clone();
        }

        if let (&Tuple(ref l1, ref r1), &Tuple(ref l2, ref r2)) = (self, other) {
            return Tuple(Rc::new(l1.sum(l2)), Rc::new(r1.sum(r2))).norm();
        }

        // one of self or other is One, this is kind of bad!
        One
    }
}

#[derive(Clone, Eq, PartialEq, Ord)]
struct Cost {
    n1: isize,
    n2: isize,
}

impl Cost {
    fn zero() -> Cost {
        Cost {
            n1: 0,
            n2: 0,
        }
    }

    fn inc1(self) -> Cost {
        Cost {
            n1: self.n1 + 1,
            n2: self.n2
        }
    }

    fn inc2(self) -> Cost {
        Cost {
            n1: 0,
            n2: self.n2 + 1
        }
    }
}

impl PartialOrd for Cost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(match self.n2.cmp(&other.n2) {
            Ordering::Equal => self.n1.cmp(&other.n1),
            x => x
        })
    }
}

#[derive(Eq, Clone)]
pub enum Event {
    Leaf(i64),
    Node(i64, Rc<Event>, Rc<Event>),
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.norm().eq_real(&other.norm())
    }
}

impl Event {
    fn eq_real(&self, other: &Event) -> bool {
        use Event::*;

        match *self {
            Leaf(n) => match *other {
                Leaf(m) => n == m,
                _ => false
            },

            Node(n, ref e1, ref e2) => match *other {
                Node(m, ref f1, ref f2) =>
                    n == m &&
                    e1.eq_real(&**f1) &&
                    e2.eq_real(&**f2),
                _ => false
            },
        }
    }

    pub fn value(&self) -> i64 {
        use Event::*;

        match *self {
            Leaf(n) => n,
            Node(n, _, _) => n
        }
    }

    pub fn lift(self, m: i64) -> Event {
        use Event::*;

        match self {
            Leaf(n) => Leaf(n + m),
            Node(n, e1, e2) => Node(n + m, e1, e2),
        }
    }

    pub fn sink(self, m: i64) -> Event {
        use Event::*;

        match self {
            Leaf(n) => Leaf(n - m),
            Node(n, e1, e2) => Node(n - m, e1, e2),
        }
    }

    pub fn min(&self) -> i64 {
        use Event::*;

        match *self {
            Leaf(n) => n,
            Node(n, ref e1, ref e2) => {
                let x1 = e1.min();
                let x2 = e2.min();
                n + if x1 < x2 { x1 } else { x2 }
            }
        }
    }

    pub fn max(&self) -> i64 {
        use Event::*;

        match *self {
            Leaf(n) => n,
            Node(n, ref e1, ref e2) => {
                let x1 = e1.max();
                let x2 = e2.max();
                n + if x1 > x2 { x1 } else { x2 }
            }
        }
    }

    pub fn norm(&self) -> Event {
        use Event::*;

        match *self {
            Leaf(n) => Leaf(n),

            Node(n, ref e1, ref e2) => {
                let f1 = e1.norm();
                let f2 = e2.norm();

                if let (&Leaf(m1), &Leaf(m2)) = (&f1, &f2) {
                    if m1 == m2 {
                        return Leaf(n + m1)
                    }
                }

                let m1 = f1.min();
                let m2 = f2.min();
                let m = if m1 < m2 { m1 } else { m2 };

                Node(n + m, Rc::new(f1.sink(m)), Rc::new(f2.sink(m)))
            },
        }
    }

    pub fn event(&self, i: &Ident) -> Event {
        let filled = self.fill(i);

        if filled != *self {
            filled
        } else {
            let (ep, _) = self.grow(i);
            ep
        }
    }

    fn fill(&self, i: &Ident) -> Event {
        use Ident::*;
        use Event::*;

        match self {
            &Leaf(n) => Leaf(n),

            &Node(n, ref el, ref er) => match i {
                &Zero => self.clone(),
                &One => Leaf(self.max()),

                &Tuple(ref il, ref ir) => {
                    if let &One = &**il {
                        let ep = er.fill(ir);
                        let ml = el.max();
                        let mr = ep.max();
                        let m = if ml > mr { ml } else { mr };
                        return Node(n, Rc::new(Leaf(m)), Rc::new(ep)).norm();
                    }

                    if let &One = &**ir {
                        let ep = el.fill(il);
                        let ml = ep.max();
                        let mr = er.max();
                        let m = if ml > mr { ml } else { mr };
                        return Node(n, Rc::new(ep), Rc::new(Leaf(m))).norm();
                    }

                    Node(n, Rc::new(el.fill(il)), Rc::new(er.fill(ir))).norm()
                }
            }
        }
    }

    fn grow(&self, i: &Ident) -> (Event, Cost) {
        use Ident::*;
        use Event::*;

        match self {
            &Leaf(n) => {
                if let &One = i {
                    (Leaf(n + 1), Cost::zero())
                } else {
                    let (e, c) = Node(
                            n, Rc::new(Leaf(0)), Rc::new(Leaf(0))
                        ).grow(i);
                    (e, c.inc2())
                }
            },

            &Node(n, ref el, ref er) => match i {
                &One | &Zero => panic!("ITC internal error!"),

                &Tuple(ref il, ref ir) => {
                    if let &Zero = &**il {
                        let (ep, c) = er.grow(ir);
                        return (Node(n, el.clone(), Rc::new(ep)), c.inc1());
                    }

                    if let &Zero = &**ir {
                        let (ep, c) = el.grow(il);
                        return (Node(n, Rc::new(ep), er.clone()), c.inc1());
                    }

                    let (elp, cl) = el.grow(il);
                    let (erp, cr) = er.grow(ir);

                    if cl < cr {
                        (Node(n, Rc::new(elp), er.clone()), cl.inc1())
                    } else {
                        (Node(n, el.clone(), Rc::new(erp)), cr.inc1())
                    }
                }
            }
        }
    }
}
